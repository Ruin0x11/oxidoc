use serde_json;

use std::fs;
use std::collections::{HashMap, HashSet};
use std::path::{PathBuf};
use std::fs::{create_dir_all, File};
use std::io::{Read, Write};

use ::errors::*;
use generator::{FnDoc, ModPath};

type FunctionName = String;

#[derive(Debug)]
pub struct StoreLoc<'a> {
    pub store: &'a Store,
    pub scope: ModPath,
    pub method: String,
}   

/// Gets the fully qualified output directory for the current module scope.
pub fn get_full_dir(store_path: &PathBuf , scope: &ModPath) -> PathBuf {
    let rest = scope.parent().to_path();

    store_path.join(rest)
}

/// Gets the .rd output file for a function's documentation
fn get_fn_file(path: &PathBuf, fn_doc: &FnDoc) -> PathBuf {
    let mut name = String::new();
    name.push_str(&fn_doc.path.name().identifier);
    name.push_str(&".rd");
    path.join(name)
}

/// A set of Rustdoc documentation for a single crate.
#[derive(Debug)]
pub struct Store {
    pub path: PathBuf,

    modules: HashSet<ModPath>,
    functions: HashMap<ModPath, FunctionName>,

    /// Documentation data in memory
    fn_docs: Vec<FnDoc>,
}

impl Store {
    pub fn new(path: PathBuf) -> Result<Store> {

        Ok(Store {
            path: path,
            modules: HashSet::new(),
            functions: HashMap::new(),

            fn_docs: Vec::new(),
        })
    }

    pub fn get_modules(&self) -> &HashSet<ModPath> {
        &self.modules
    }

    /// Load the cache for this store, which currently contains the names of all modules.
    pub fn load_cache(&mut self) -> Result<()> {
        let path = self.path.join("cache.rd");

        let mut fp = File::open(&path)
            .chain_err(|| format!("Couldn't find rd cache {}", &path.display()))?;

        let mut json = String::new();
        fp.read_to_string(&mut json)
            .chain_err(|| format!("Couldn't read rd cache {}", &path.display()))?;

        info!("rd: {}", &path.display());
        let module_names: HashSet<ModPath> = serde_json::from_str(&json).unwrap();
        info!("MN: {:?}", &module_names);
        self.modules = module_names;

        Ok(())
    }

    /// Attempt to load the method at 'loc' from the store.
    pub fn load_method(&self, loc: StoreLoc) -> Result<FnDoc> {
        info!("Looking for {} in store {} ", loc.scope, &self.path.display());
        let doc_path = self.path.join(loc.scope.to_path())
            .join(format!("{}.rd", loc.method));
        let mut fp = File::open(&doc_path)
            .chain_err(|| format!("Couldn't find rd store {}", doc_path.display()))?;

        let mut json = String::new();
        fp.read_to_string(&mut json)
            .chain_err(|| format!("Couldn't read rd store {}", doc_path.display()))?;

        info!("Loading {}", doc_path.display());
        let fn_doc: FnDoc = serde_json::from_str(&json).unwrap();

        info!("{}\n{}", fn_doc.to_string(), fn_doc.signature);

        Ok(fn_doc)
    }

    /// Adds a function's info to the store in memory.
    pub fn add_fn(&mut self, fn_doc: FnDoc) {
        self.modules.insert(fn_doc.path.parent());
        self.functions.insert(fn_doc.path.parent(),
                              fn_doc.path.name().identifier.clone());
        info!("Module {} contains fn {}", fn_doc.path.parent().to_string(),
                 fn_doc.path.name().identifier);

        self.fn_docs.push(fn_doc);

    }

    /// Writes a .rd JSON store documenting a function to disk.
    pub fn save_fn(&self, fn_doc: &FnDoc) -> Result<PathBuf> {
        let json = serde_json::to_string(&fn_doc).unwrap();
        let full_path = get_full_dir(&self.path, &fn_doc.path);

        create_dir_all(&full_path).chain_err(|| format!("Failed to create module dir {}", self.path.display()))?;

        let outfile = get_fn_file(&full_path, &fn_doc);

        let mut fp = File::create(&outfile).chain_err(|| format!("Could not write method rd file {}", outfile.display()))?;
        fp.write_all(json.as_bytes()).chain_err(|| format!("Failed to write to method rd file {}", outfile.display()))?;

        // Insert the module name into the list of known module names

        info!("Wrote fn doc to {}", &outfile.display());

        Ok(outfile)
    }

    /// Saves all documentation data that is in-memory to disk.
    pub fn save(&self) -> Result<()> {
        fs::create_dir_all(&self.path)
            .chain_err(|| format!("Unable to create directory {}", &self.path.display()))?;

        self.save_cache()
            .chain_err(|| format!("Unable to save cache for directory {}", &self.path.display()))?;

        for fn_doc in &self.fn_docs {
            self.save_fn(&fn_doc)
                .chain_err(|| format!("Cannot save fn doc {}", &fn_doc.path.name()))?;
        }

        Ok(())
    }

    /// Saves this store's cached list of module names to disk.
    pub fn save_cache(&self) -> Result<()> {
        let json = serde_json::to_string(&self.modules).unwrap();

        let outfile = self.path.join("cache.rd");
        let mut fp = File::create(&outfile).chain_err(|| format!("Could not write cache file {}", outfile.display()))?;
        fp.write_all(json.as_bytes()).chain_err(|| format!("Failed to write to method rd file {}", outfile.display()))?;

        Ok(())
    }
}
