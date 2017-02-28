use serde_json;

use std::collections::{HashMap, HashSet};
use std::path::{PathBuf};
use std::fs::{create_dir_all, File};
use std::io::{Read, Write};

use ::errors::*;
use generator::{FnDoc, ModPath};

type ProjectName = String;
type ModuleName = String;
type FunctionName = String;

pub struct StoreLoc<'a> {
    pub store: &'a Store,
    pub scope: ModPath,
    pub method: String,
}   

/// Gets the fully qualified output directory for the current module scope.
pub fn get_full_dir(store_path: &PathBuf , scope: &ModPath) -> PathBuf {
    let rest = scope.to_path();

    store_path.join(rest)
}

fn get_fn_file(path: &PathBuf, fn_doc: &FnDoc) -> PathBuf {
    let mut first = String::new();
    first.push_str(&fn_doc.path.0.iter().next().unwrap().identifier);
    let mut result = fn_doc.path.0.iter().skip(1)
        .fold(first, |res, s| res + "-" + &s.identifier);
    result.push_str(".rd");
    path.join(result)
}

/// A set of Rustdoc documentation for a single crate.
pub struct Store {
    pub path: PathBuf,
    name: ProjectName,

    modules: HashSet<ModuleName>,
    modules_containing_fns: HashMap<FunctionName, ModuleName>,
}

impl Store {
    pub fn new(path: PathBuf) -> Result<Store> {

        Ok(Store {
            path: path,
            name: String::new(),
            modules: HashSet::new(),
            modules_containing_fns: HashMap::new(),
        })
    }

    pub fn friendly_path(&self) -> String {
        let s = String::new();
        let last = self.path.iter().last().unwrap();
        // s.push_str(last.as_str());
        s
    }

    pub fn get_modules(&self) -> &HashSet<ModuleName> {
        &self.modules
    }

    pub fn load_cache(&mut self) -> Result<()> {
        let path = self.path.join("cache.rd");

        let mut fp = File::open(&path)
            .chain_err(|| format!("Couldn't find rd cache {}", &path.display()))?;

        let mut json = String::new();
        fp.read_to_string(&mut json)
            .chain_err(|| format!("Couldn't read rd cache {}", &path.display()))?;

        let module_names: HashSet<ModuleName> = serde_json::from_str(&json).unwrap();
        println!("MN: {:?}", &module_names);
        self.modules = module_names;

        Ok(())
    }

    pub fn load_method(&self, loc: StoreLoc) -> Result<FnDoc> {
        let doc_path = self.path.join(loc.scope.to_path())
            .join(format!("{}.rd", loc.method));
        let mut fp = File::open(&doc_path)
            .chain_err(|| format!("Couldn't find rd store {}", doc_path.display()))?;

        let mut json = String::new();
        fp.read_to_string(&mut json)
            .chain_err(|| format!("Couldn't read rd store {}", doc_path.display()))?;

        let fn_doc: FnDoc = serde_json::from_str(&json).unwrap();

        println!("{}\n{}", fn_doc.to_string(), fn_doc.signature);

        Ok(fn_doc)
    }

    /// Writes a .rd JSON store documenting a function.
    pub fn write_fn(&mut self, fn_doc: FnDoc) -> Result<PathBuf> {
        let json = serde_json::to_string(&fn_doc).unwrap();
        let full_path = get_full_dir(&self.path, &fn_doc.path);

        create_dir_all(&full_path).chain_err(|| format!("Failed to create module dir {}", self.path.display()))?;

        let outfile = get_fn_file(&full_path, &fn_doc);

        let mut fp = File::create(&outfile).chain_err(|| format!("Could not write method rd file {}", outfile.display()))?;
        fp.write_all(json.as_bytes()).chain_err(|| format!("Failed to write to method rd file {}", outfile.display()))?;

        // Insert the module name into the list of known module names
        self.modules.insert(fn_doc.path.parent().to_string());
        self.modules_containing_fns.insert(fn_doc.path.name().identifier.clone(),
                                           fn_doc.path.parent().to_string());
        println!("Module {} contains {}", fn_doc.path.name().identifier,
                 fn_doc.path.parent().to_string());

        // println!("Wrote {}", &outfile.display());

        Ok(outfile)
    }

    pub fn save_cache(&self) -> Result<()> {
        let json = serde_json::to_string(&self.modules).unwrap();

        let outfile = self.path.join("cache.rd");
        let mut fp = File::create(&outfile).chain_err(|| format!("Could not write cache file {}", outfile.display()))?;
        fp.write_all(json.as_bytes()).chain_err(|| format!("Failed to write to method rd file {}", outfile.display()))?;

        Ok(())
    }

    fn lookup_method(&self, name: String) -> Result<String> {
        bail!("as");
    }

    //     fn find_methods(&self, name: String) -> Result<Vec<FnDoc>> {
    //         // TODO: no caching/lookup is used.
    //         let mut paths = Vec::new();
    //         for module in self.modules {
    //             let mod_path = ::ModPath::from(name);
    //             let full_dir = get_full_dir(&self.path, &mod_path);

    //             let mut doc_paths = read_dir(full_dir)
    //                 .chain_err(|| "Couldn't read doc path")?;


    //             // Recursively walk over doc directory
    //         //     match walk_dir("a") {
    //         //         Err(why) => bail!(format!("! {:?}", why.kind())),
    //         //         Ok(doc_paths) => for doc in doc_paths {
    //         //             if let Ok(doc_path) = doc {
    //         //                 if let Ok(metadata) = doc_path.metadata() {
    //         //                     if metadata.is_file()
    //         //                         && doc_path.path().extension().unwrap() == OsStr::new("rd") {
    //         //                             paths.push(doc_path.path());
    //         //                         }
    //         //                 }
    //         //             }
    //         //         },
    //         //     }
    //         // }

    //         // let results = paths.iter().map(|p| load_method(p).unwrap()).collect::<Vec<FnDoc>>();

    //     }
             // Ok(results);
}
