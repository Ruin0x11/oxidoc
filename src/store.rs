use serde_json;

use std::collections::{HashMap, HashSet};
use std::path::{PathBuf};
use std::fs::{self, create_dir_all, File};
use std::fmt::Display;
use std::io::{Read, Write};

use ::errors::*;
use document::{Document, FnDoc, StructDoc, ModPath};
use paths;

#[derive(Debug)]
pub struct StoreLoc<'a> {
    pub store: &'a Store,
    pub scope: ModPath,
    pub identifier: String,
}   

/// Gets the fully qualified output directory for the current module scope.
pub fn get_full_dir(store_path: &PathBuf , scope: &ModPath) -> PathBuf {
    let rest = scope.parent().unwrap().to_string();

    store_path.join(rest)
}

/// Gets the .odoc output file for a function's documentation
fn get_fn_docfile(store_path: &PathBuf, fn_doc: &FnDoc) -> Result<PathBuf> {
    let parent = fn_doc.path.parent().unwrap().to_path();
    let docfile = paths::encode_doc_filename(&fn_doc.path.name().unwrap().identifier)
        .chain_err(|| "Could not encode doc filename")?;
    let name = format!("{}.odoc", docfile);
    let local_path = parent.join(name);
    Ok(store_path.join(local_path))
}

/// Gets the .odoc output file for a function's documentation
fn get_struct_docfile(store_path: &PathBuf, struct_doc: &StructDoc) -> Result<PathBuf> {
    let parent = struct_doc.path.parent().unwrap().to_path();
    let docfile = paths::encode_doc_filename(&struct_doc.path.name().unwrap().identifier)
        .chain_err(|| "Could not encode doc filename")?;
    let name = format!("sdesc-{}.odoc", docfile);
    let local_path = parent.join(name);
    Ok(store_path.join(local_path))
}

type FunctionName = String;
type StructName = String;

/// A set of Rustdoc documentation for a single crate.
#[derive(Debug)]
pub struct Store {
    pub path: PathBuf,

    /// Locations of documentation in the store
    modules: HashSet<ModPath>,
    functions: HashMap<ModPath, HashSet<FunctionName>>,
    structs: HashMap<ModPath, HashSet<StructName>>,

    /// Documentation data in memory
    fn_docs: Vec<FnDoc>,
    struct_docs: Vec<StructDoc>,
}

impl Store {
    pub fn new(path: PathBuf) -> Result<Store> {

        Ok(Store {
            path: path,
            modules: HashSet::new(),
            functions: HashMap::new(),
            structs: HashMap::new(),

            fn_docs: Vec::new(),
            struct_docs: Vec::new(),
        })
    }

    pub fn get_functions(&self, scope: &ModPath) -> Option<&HashSet<FunctionName>> {
        self.functions.get(scope)
    }

    pub fn get_structs(&self, scope: &ModPath) -> Option<&HashSet<StructName>> {
        self.structs.get(scope)
    }

    pub fn get_modules(&self) -> &HashSet<ModPath> {
        for m in &self.modules {
            info!("module: {}\n", m);
        }
        &self.modules
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
        self.modules = module_names;

        Ok(())
    }

    /// Attempt to load the function at 'loc' from the store.
    pub fn load_function(&self, scope: &ModPath, identifier: &String) -> Result<FnDoc> {
        let encoded_name = paths::encode_doc_filename(&identifier)
            .chain_err(|| format!("Failed to encode StoreLoc identifier {}", identifier))?;
        let doc_path = self.path.join(scope.to_path())
            .join(format!("{}.odoc", encoded_name));
        info!("Looking for {}", &doc_path.display());
        let mut fp = File::open(&doc_path)
            .chain_err(|| format!("Couldn't find oxidoc store {}", doc_path.display()))?;

        let mut json = String::new();
        fp.read_to_string(&mut json)
            .chain_err(|| format!("Couldn't read oxidoc store {}", doc_path.display()))?;

        info!("Loading {}", doc_path.display());
        let fn_doc: FnDoc = serde_json::from_str(&json)
            .chain_err(|| "Couldn't parse oxidoc store (regen probably needed)")?;

        Ok(fn_doc)
    }

    /// Attempt to load the struct at 'loc' from the store.
    pub fn load_struct(&self, scope: &ModPath, identifier: &String) -> Result<StructDoc> {
        let encoded_name = paths::encode_doc_filename(&identifier)
            .chain_err(|| format!("Failed to encode StoreLoc identifier {}", identifier))?;

        // The struct documentation will live inside that struct's folder, so make sure to join it.
        let doc_path = self.path.join(scope.to_path())
            .join(format!("{}/sdesc-{}.odoc", encoded_name, encoded_name));

        info!("Looking for {}", &doc_path.display());

        let mut fp = File::open(&doc_path)
            .chain_err(|| format!("Couldn't find oxidoc store {}", doc_path.display()))?;

        let mut json = String::new();
        fp.read_to_string(&mut json)
            .chain_err(|| format!("Couldn't read oxidoc store {}", doc_path.display()))?;

        info!("Loading {}", doc_path.display());
        let struct_doc: StructDoc = serde_json::from_str(&json)
            .chain_err(|| "Couldn't parse oxidoc store (regen probably needed)")?;

        Ok(struct_doc)
    }

    /// Adds a function's info to the store in memory.
    pub fn add_function(&mut self, fn_doc: FnDoc) {
        let parent = fn_doc.path.parent().unwrap();
        self.add_module(parent.clone());

        if let Some(list) = self.functions.get_mut(&parent) {
            let identifier = fn_doc.path.name().unwrap().identifier.clone();
            list.insert(identifier);
        }
        if let None = self.functions.get(&parent) {
            let identifier = fn_doc.path.name().unwrap().identifier.clone();
            let mut s = HashSet::new();
            s.insert(identifier);
            self.functions.insert(parent, s);
        }

        info!("Module {} contains fn {}", fn_doc.path.parent().unwrap().to_string(),
                 fn_doc.path.name().unwrap().identifier);

        self.fn_docs.push(fn_doc);
    }

    /// Add a module's path to the list of known modules in this store.
    pub fn add_module(&mut self, scope: ModPath) {
        info!("Add Module: {}", scope);
        self.modules.insert(scope);
    }

    fn add_all_modules(&mut self, scope: &ModPath) {
        let mut parent = scope.parent();
        while let Some(path) = parent {
            info!("Add module path {}", &path);
            parent = path.parent();
            self.modules.insert(path);
        }
    }

    /// Adds a struct's info to the store in memory.
    pub fn add_struct(&mut self, struct_doc: StructDoc) {
        let parent = struct_doc.path.parent().unwrap();

        if let Some(list) = self.structs.get_mut(&parent) {
            let identifier = struct_doc.path.name().unwrap().identifier.clone();
            list.insert(identifier);
        }
        if let None = self.structs.get(&parent) {
            let identifier = struct_doc.path.name().unwrap().identifier.clone();
            let mut s = HashSet::new();
            s.insert(identifier);
            self.structs.insert(parent, s);
        }

        info!("Module {} contains struct {}", struct_doc.path.parent().unwrap().to_string(),
                 struct_doc.path.name().unwrap().identifier);

        self.struct_docs.push(struct_doc);
    }

    /// Writes a .odoc JSON store documenting a struct to disk.
    pub fn save_struct(&self, struct_doc: &StructDoc) -> Result<PathBuf> {
        let json = serde_json::to_string(&struct_doc).unwrap();

        let outfile = get_struct_docfile(&self.path, &struct_doc)
            .chain_err(|| format!("Could not obtain docfile path inside {}", self.path.display()))?;

        create_dir_all(outfile.parent().unwrap())
            .chain_err(|| format!("Failed to create module dir {}", self.path.display()))?;

        let mut fp = File::create(&outfile)
            .chain_err(|| format!("Could not write struct odoc file {}", outfile.display()))?;
        fp.write_all(json.as_bytes())
            .chain_err(|| format!("Failed to write to struct odoc file {}", outfile.display()))?;

        // Insert the module name into the list of known module names

        info!("Wrote struct doc to {}", &outfile.display());

        Ok(outfile)
    }

    /// Writes a .odoc JSON store documenting a function to disk.
    pub fn save_fn(&self, fn_doc: &FnDoc) -> Result<PathBuf> {
        let json = serde_json::to_string(&fn_doc).unwrap();

        let outfile = get_fn_docfile(&self.path, &fn_doc)
            .chain_err(|| format!("Could not obtain docfile path inside {}", self.path.display()))?;

        create_dir_all(outfile.parent().unwrap())
            .chain_err(|| format!("Failed to create module dir {}", self.path.display()))?;

        let mut fp = File::create(&outfile)
            .chain_err(|| format!("Could not write function odoc file {}", outfile.display()))?;
        fp.write_all(json.as_bytes())
            .chain_err(|| format!("Failed to write to function odoc file {}", outfile.display()))?;

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

        for struct_doc in &self.struct_docs {
            self.save_struct(&struct_doc)
                .chain_err(|| format!("Could not save struct doc {}", &struct_doc.path.name().unwrap()))?;
        }

        for fn_doc in &self.fn_docs {
            self.save_fn(&fn_doc)
                .chain_err(|| format!("Could not save function doc {}", &fn_doc.path.name().unwrap()))?;
        }

        Ok(())
    }

    /// Saves this store's cached list of module names to disk.
    pub fn save_cache(&self) -> Result<()> {
        let json = serde_json::to_string(&self.modules).unwrap();

        let outfile = self.path.join("cache.odoc");
        let mut fp = File::create(&outfile).chain_err(|| format!("Could not write cache file {}", outfile.display()))?;
        fp.write_all(json.as_bytes()).chain_err(|| format!("Failed to write to function odoc file {}", outfile.display()))?;

        Ok(())
    }
}
