use syntax::ast;
use syntax::print::pprust;

use serde_json;

use std::env;
use std::fs::create_dir_all;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::fs::{File};
use std::io::{Read, Write};

use ::errors::*;
use ::FnDoc;
use ::ModPath;
use ::PathSegment;

type ProjectName = String;
type ModuleName = String;
type FunctionName = String;

/// Contains all information for a single project
pub struct Store {
    outdir: PathBuf,
    name: ProjectName,

    modules: Vec<ModuleName>,
    functions: HashMap<ModuleName, FunctionName>,

    current_scope: ::ModPath,
}

impl Store {
    pub fn new(crate_info: &::CrateInfo) -> Result<Store> {
        let crate_doc_path = get_crate_doc_path(&crate_info)
            .chain_err(|| format!("Unable to get crate doc path for crate: {}", crate_info.package.name))?;

        Ok(Store {
            outdir: crate_doc_path,
            name: crate_info.package.name.clone(),
            modules: Vec::new(),
            functions: HashMap::new(),
            current_scope: ModPath(Vec::new()),
        })
    }

    #[cfg(never)]
    pub fn load_cache(&mut self) -> Result<()> {
        let path =  self.outdir.as_path();
        let mut fp = File::open(path)
            .chain_err(|| format!("Couldn't find rd store {}", path.display()))?;

        let mut json = String::new();
        fp.read_to_string(&mut json)
            .chain_err(|| format!("Couldn't read rd store {}", path.display()))?;

        let fn_docs: Vec<FnDoc> = serde_json::from_str(&json)
            .chain_err(|| format!("Store {} is not valid JSON", path.display()))?;

        for func in &fn_docs {
            self.functions.insert(func.to_string(), func.signature);
            println!("{}", func.signature);
        }

        Ok(())
    }

    fn get_full_outdir(&self) -> PathBuf {
        let rest = self.current_scope.0.iter().fold(String::new(), |res, s| res + &s.identifier.clone() + "/");

        println!("full outdir: {}", rest);
        self.outdir.join(rest)
    }

    pub fn write_fn(&self, fn_doc: FnDoc) -> Result<PathBuf> {
        let json = serde_json::to_string(&fn_doc).unwrap();
        let full_outdir = self.get_full_outdir();

        create_dir_all(&full_outdir).chain_err(|| format!("Failed to create module dir {}", self.outdir.display()))?;

        let outfile = get_fn_file(&full_outdir, &fn_doc);

        let mut fp = File::create(&outfile).chain_err(|| format!("Could not method rd file {}", outfile.display()))?;
        fp.write_all(json.as_bytes()).chain_err(|| format!("Failed to write to method rd file {}", outfile.display()))?;

        println!("Wrote {}", &outfile.display());

        Ok(outfile)
    }

    pub fn push_path(&mut self, ident: ast::Ident) {
        let seg = PathSegment{ identifier: pprust::ident_to_string(ident) };
        println!("push {}", seg.identifier);
        self.current_scope.push(seg);
    }
    pub fn pop_path(&mut self) {
        println!("pop");
        self.current_scope.pop();
    }
}

fn get_crate_doc_path(crate_info: &::CrateInfo) -> Result<PathBuf> {
    let home_dir: PathBuf;
    if let Some(x) = env::home_dir() {
        home_dir = x
    } else {
        bail!("Could not locate home directory");
    }

    let path = home_dir.as_path().join(".cargo/registry/doc")
        .join(format!("{}-{}", crate_info.package.name, crate_info.package.version));
    Ok(path)
}

fn get_fn_file(path: &PathBuf, fn_doc: &FnDoc) -> PathBuf {
    let mut first = String::new();
    first.push_str(&fn_doc.path.0.iter().next().unwrap().identifier);
    let mut result = fn_doc.path.0.iter().skip(1)
        .fold(first, |res, s| res + "-" + &s.identifier);
    result.push_str(".rd");
    path.join(result)
}

// fn lookup_method(store: Store, name: String) -> Result<String> {

// }
