use serde_json;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::fs::{File};
use std::io::{Read};
use ::errors::*;
use ::FnDoc;

type ModuleName = String;
type FunctionName = String;

/// Contains all information for a single module
pub struct Store {
    filepath: PathBuf,
    name: String,

    modules: Vec<ModuleName>,

    functions: HashMap<ModuleName, FunctionName>,
}

impl Store {
    pub fn new(filepath: PathBuf, name: String) -> Store {
        Store {
            filepath: filepath,
            name: name,
            modules: Vec::new(),
            functions: HashMap::new(),
        }
    }

    pub fn load_cache(&mut self) -> Result<()> {
        let path =  self.filepath.as_path();
        let mut fp = File::open(path)
            .chain_err(|| format!("Couldn't find rd store {}", path.display()))?;

        let mut json = String::new();
        fp.read_to_string(&mut json)
            .chain_err(|| format!("Couldn't read rd store {}", path.display()))?;

        let fn_docs: Vec<FnDoc> = serde_json::from_str(&json)
            .chain_err(|| format!("Store {} is not valid JSON", path.display()))?;

        for func in &fn_docs {
            println!("{}", func.signature);
        }

        Ok(())
    }
}

// fn lookup_method(store: Store, name: String) -> Result<String> {
    
// }
