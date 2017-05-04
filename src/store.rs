use convert::NewDocTemp_;
use std::collections::HashMap;
use std::env;
use std::fmt;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use bincode::{self, Infinite};
use serde::de::Deserialize;
use serde::ser::Serialize;

use convert::DocType;
use document::CrateInfo;
use document::ModPath;
use ::errors::*;

const STORE_FILENAME: &str = "store.odoc";

pub fn get_doc_registry_path() -> Result<PathBuf> {
    let home_dir = if let Some(dir) = env::home_dir() {
        dir
    } else {
        bail!("Could not locate home directory");
    };

    Ok(home_dir.as_path().join(".cargo/registry/doc"))
}

/// Obtains the base output path for a crate's documentation.
pub fn get_crate_doc_path(crate_info: &CrateInfo) -> Result<PathBuf> {
    let registry_path = get_doc_registry_path()?;

    let path = registry_path.join(format!("{}-{}",
                            crate_info.name,
                            crate_info.version));
    Ok(path)
}


fn get_store_file() -> Result<PathBuf> {
    let mut registry_path = get_doc_registry_path()?;
    registry_path.push(STORE_FILENAME);
    Ok(registry_path)
}

fn create_or_open_file<T: AsRef<Path>>(path: T) -> Result<File> {
    let path_as = path.as_ref();
    if !path_as.exists() {
        OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(path_as)
            .chain_err(|| format!("Could not create file {}", path_as.display()))
    } else {
        OpenOptions::new()
            .read(true)
            .write(true)
            .open(path_as)
            .chain_err(|| format!("Could not open file {}", path_as.display()))
    }
}

pub fn read_bincode_data<S, T>(path: T) -> Result<S>
    where S: Deserialize,
          T: AsRef<Path>
{
    let path_as = path.as_ref();
    let mut data: Vec<u8> = Vec::new();
    let mut bincoded_file = create_or_open_file(path_as)?;

    bincoded_file.read_to_end(&mut data)
        .chain_err(|| format!("Failed to read file {}", path_as.display()))?;
    let result = bincode::deserialize(&data)
        .chain_err(|| format!("Could not deserialize file at {}", path_as.display()))?;

    Ok(result)
}

pub fn write_bincode_data<S, T>(data: S, path: T) -> Result<()>
    where S: Serialize,
          T: AsRef<Path>
{
    let path_as = path.as_ref();

    let data = bincode::serialize(&data, Infinite)
        .chain_err(|| format!("Could not deserialize file at {}", path_as.display()))?;

    let mut bincoded_file = create_or_open_file(path_as)?;
    bincoded_file.write(data.as_slice())
        .chain_err(|| format!("Failed to write file {}", path_as.display()))?;

    Ok(())
}

#[derive(Serialize, Deserialize)]
pub struct Store {
    items: HashMap<CrateInfo, Docset>,
}

pub struct StoreLocation {
    pub crate_info: CrateInfo,
    pub mod_path: ModPath,
}

impl StoreLocation {
    pub fn new(crate_info: CrateInfo, mod_path: ModPath) -> Self {
        StoreLocation {
            crate_info: crate_info,
            mod_path: mod_path,
        }
    }

    pub fn to_filepath(&self) -> PathBuf {
        let mut path = self.crate_info.to_path_prefix();
        path.push(self.mod_path.to_filepath());
        path
    }
}

impl fmt::Display for StoreLocation {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} ({} {})", self.mod_path, self.crate_info.name, self.crate_info.version)
    }
}

impl Store {
    pub fn new() -> Self {
        Store {
            items: HashMap::new(),
        }
    }

    pub fn load() -> Self {
        match Store::load_from_disk() {
            Ok(store) => store,
            Err(_)    => Store::new(),
        }
    }

    pub fn save(&mut self) -> Result<()> {
        let store_file = get_store_file()?;
        write_bincode_data(self, store_file)
    }

    pub fn load_from_disk() -> Result<Self> {
        let store_file = get_store_file()?;
        read_bincode_data(store_file)
    }

    pub fn add_docset(&mut self, crate_info: CrateInfo, docset: Docset) {
        self.items.insert(crate_info, docset);
    }

    pub fn all_locations(&self) -> Vec<StoreLocation> {
        let mut results = Vec::new();
        for docset in self.items.values() {
            for paths in docset.documents.values() {
                for path in paths.iter() {
                    results.push(StoreLocation::new(docset.crate_info.clone(), path.clone()));
                }
            }
        }
        results
    }
}

#[derive(Serialize, Deserialize)]
pub struct Docset {
    pub crate_info: CrateInfo,
    pub documents: HashMap<DocType, Vec<ModPath>>,
}

impl Docset {
    pub fn new(crate_info: CrateInfo) -> Self {
        Docset {
            crate_info: crate_info,
            documents: HashMap::new(),
        }
    }

    pub fn add_docs(&mut self, documents: Vec<NewDocTemp_>) -> Result<()> {
        for doc in documents.into_iter() {
            let entry = self.documents.entry(doc.get_type()).or_insert(Vec::new());
            entry.push(doc.mod_path.clone());
            doc.save(&self.crate_info)
                .chain_err(|| format!("Could not add doc {} to docset", doc.mod_path))?;
        }
        Ok(())
    }
}
