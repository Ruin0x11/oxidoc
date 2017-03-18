use serde::ser::{Serialize};
use serde::de::{Deserialize};
use serde_json;
use document::Documentable;

use std::collections::{HashMap, HashSet};
use std::path::{PathBuf};
use std::fs::{self, File};
use std::io::{Read, Write};

use ::errors::*;
use document::*;

/// Defines an exact location a documentation file can be found.
#[derive(Debug)]
pub struct StoreLoc<'a> {
    pub store: &'a Store,
    pub path: ModPath,
}   

/// Gets the fully qualified output directory for the current module scope.
pub fn get_full_dir(store_path: &PathBuf , scope: &ModPath) -> PathBuf {
    let rest = scope.parent().unwrap().to_string();

    store_path.join(rest)
}

type FunctionName = String;
type StructName = String;

/// A set of Rustdoc documentation for a single crate.
#[derive(Debug)]
pub struct Store {
    pub name: String,
    pub path: PathBuf,

    // Locations of documentation in the store
    modpaths: HashSet<ModPath>,
    functions: HashMap<ModPath, HashSet<FunctionName>>,
    structs: HashMap<ModPath, HashSet<StructName>>,
}

impl Store {
    pub fn new(path: PathBuf) -> Store {
        Store {
            name: "".to_string(),
            path: path,
            modpaths: HashSet::new(),
            functions: HashMap::new(),
            structs: HashMap::new(),
        }
    }

    pub fn get_functions(&self, scope: &ModPath) -> Option<&HashSet<FunctionName>> {
        self.functions.get(scope)
    }

    pub fn get_structs(&self, scope: &ModPath) -> Option<&HashSet<StructName>> {
        self.structs.get(scope)
    }

    pub fn get_modpaths(&self) -> &HashSet<ModPath> {
        for m in &self.modpaths {
            info!("module: {}\n", m);
        }
        &self.modpaths
    }

    /// Load the cache for this store, which currently contains the names of all modules.
    pub fn load_cache(&mut self) -> Result<()> {
        let path = self.path.join("cache.odoc");

        let mut fp = File::open(&path)
            .chain_err(|| format!("Couldn't find oxidoc cache {}", &path.display()))?;

        let mut json = String::new();
        fp.read_to_string(&mut json)
            .chain_err(|| format!("Couldn't read oxidoc cache {}", &path.display()))?;

        info!("odoc: {}", &path.display());
        let module_names: HashSet<ModPath> = serde_json::from_str(&json).unwrap();
        info!("MN: {:?}", &module_names);
        self.modpaths = module_names;

        Ok(())
    }
    /// Add a module's path to the list of known modules in this store.
    pub fn add_modpath(&mut self, scope: ModPath) {
        self.modpaths.insert(scope);
    }

    fn add_all_modpaths(&mut self, scope: &ModPath) {
        let mut parent = scope.parent();
        while let Some(path) = parent {
            parent = path.parent();
            self.modpaths.insert(path);
        }
    }

    pub fn load_doc<T: Documentable + Serialize + Deserialize>(&self, doc_path: &ModPath) -> Result<Document<T>> {
        info!("Store path: {}, Doc path: {}", &self.path.display(), &doc_path);
        match Document::load_doc(self.path.clone(), doc_path) {
            Ok(doc) => Ok(doc),
            Err(e) => Err(e)
        }
    }
    /// Saves all documentation data that is in-memory to disk.
    pub fn save(&self) -> Result<()> {
        fs::create_dir_all(&self.path)
            .chain_err(|| format!("Unable to create directory {}", &self.path.display()))?;

        self.save_cache()
            .chain_err(|| format!("Unable to save cache for directory {}", &self.path.display()))?;

        // TODO: save the rest of documentation

        Ok(())
    }

    /// Saves this store's cached list of module names to disk.
    pub fn save_cache(&self) -> Result<()> {
        let json = serde_json::to_string(&self.modpaths).unwrap();

        let outfile = self.path.join("cache.odoc");
        let mut fp = File::create(&outfile).chain_err(|| format!("Could not write cache file {}", outfile.display()))?;
        fp.write_all(json.as_bytes()).chain_err(|| format!("Failed to write to function odoc file {}", outfile.display()))?;

        Ok(())
    }
}
