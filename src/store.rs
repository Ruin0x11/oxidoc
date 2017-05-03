use convert::NewDocTemp_;
use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use bincode::{self, Infinite};
use serde::de::Deserialize;
use serde::ser::Serialize;

use convert::DocType;
use document::CrateInfo;
use document::ModPath;
use ::errors::*;

pub const CARGO_DOC_PATH: &str = ".cargo/registry/doc";

pub fn read_bincode_data<S, T>(path: T) -> Result<S>
    where S: Deserialize,
          T: AsRef<Path>
{
    let path_as = path.as_ref();
    let mut data: Vec<u8> = Vec::new();
    let mut bincoded_file = File::open(path_as)
        .chain_err(|| format!("No such file {}", path_as.display()))?;

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

    let mut bincoded_file = File::create(path_as)
        .chain_err(|| format!("Failed to create file {}", path_as.display()))?;
    bincoded_file.write(data.as_slice())
        .chain_err(|| format!("Failed to write file {}", path_as.display()))?;

    Ok(())
}

#[derive(Serialize, Deserialize)]
pub struct Store {
    items: HashMap<CrateInfo, Docset>,
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
        write_bincode_data(self, PathBuf::from(format!("{}/cache.odoc", CARGO_DOC_PATH)))
    }

    pub fn load_from_disk() -> Result<Self> {
        read_bincode_data(PathBuf::from(format!("{}/cache.odoc", CARGO_DOC_PATH)))
    }

    pub fn add_docset(&mut self, crate_info: CrateInfo, docset: Docset) {
        self.items.insert(crate_info, docset);
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
