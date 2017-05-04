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

const STORE_FILENAME: &str = "store";

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

pub fn deserialize_object<S, T>(path: T) -> Result<S>
    where S: Deserialize,
          T: AsRef<Path>
{
    let path_as = path.as_ref();
    let mut data: Vec<u8> = Vec::new();
    let mut bincoded_file = File::open(&path_as)
        .chain_err(|| format!("Could not open file {}", path_as.display()))?;

    bincoded_file.read_to_end(&mut data)
        .chain_err(|| format!("Failed to read file {}", path_as.display()))?;
    let result = bincode::deserialize(data.as_slice())
        .chain_err(|| format!("Could not deserialize file at {}", path_as.display()))?;

    Ok(result)
}

pub fn serialize_object<S, T>(data: &S, path: T) -> Result<()>
    where S: Serialize,
          T: AsRef<Path>
{
    let path_as = path.as_ref();

    let data = bincode::serialize(data, Infinite)
        .chain_err(|| format!("Could not serialize data for {}", path_as.display()))?;

    let mut bincoded_file = create_or_open_file(path_as)?;
    bincoded_file.write(data.as_slice())
        .chain_err(|| format!("Failed to write file {}", path_as.display()))?;

    Ok(())
}

#[derive(Serialize, Deserialize)]
pub struct Store {
    items: HashMap<CrateInfo, Docset>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct StoreLocation {
    pub name: String,
    pub crate_info: CrateInfo,
    pub mod_path: ModPath,
    pub doc_type: DocType,
}

impl StoreLocation {
    pub fn new(name: String,
               crate_info: CrateInfo,
               mod_path: ModPath,
               doc_type: DocType) -> Self
    {
        StoreLocation {
            name: name,
            crate_info: crate_info,
            mod_path: mod_path,
            doc_type: doc_type,
        }
    }

    pub fn to_filepath(&self) -> PathBuf {
        let mut path = get_crate_doc_path(&self.crate_info).unwrap();
        let doc_path = self.mod_path.to_filepath();
        path.push(doc_path);
        let filename = format!("{}{}.odoc", self.doc_type.get_file_prefix(), self.name);
        path.push(filename);
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
        serialize_object(self, store_file)
    }

    pub fn load_from_disk() -> Result<Self> {
        let store_file = get_store_file()?;
        deserialize_object(store_file)
    }

    pub fn add_docset(&mut self, crate_info: CrateInfo, docset: Docset) {
        self.items.insert(crate_info, docset);
    }

    pub fn all_locations(&self) -> Vec<StoreLocation> {
        let mut results = Vec::new();
        for docset in self.items.values() {
            results.extend(docset.documents.clone());
        }
        results
    }
}

#[derive(Serialize, Deserialize)]
pub struct Docset {
    pub documents: Vec<StoreLocation>,
}

impl Docset {
    pub fn new() -> Self {
        Docset {
            documents: Vec::new(),
        }
    }

    pub fn add_docs(&mut self, documents: Vec<NewDocTemp_>) -> Result<()> {
        for doc in documents.into_iter() {
            self.documents.push(doc.to_store_location());
            doc.save()
                .chain_err(|| format!("Could not add doc {} to docset", doc.mod_path))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_write_bincode() {
        let string = "Test.".to_string();
        let path = PathBuf::from("/tmp/test.txt");

        serialize_object(&string, &path).expect("Write failed");
        let result: String = deserialize_object(&path).expect("Read failed");

        assert_eq!(string, result);
    }

    #[test]
    fn test_store_loc_to_path() {
        let loc = StoreLocation {
            name: "TEST".to_string(),
            crate_info: CrateInfo {
                name: "test".to_string(),
                version: "0.1.0".to_string(),
            },
            mod_path: ModPath::from("{{root}}::crate::mod".to_string()),
            doc_type: DocType::Const,
        };

        assert_eq!(loc.to_filepath(), PathBuf::from("test-0.1.0/crate/mod/TEST/cdesc-TEST.odoc"));
    }
}
