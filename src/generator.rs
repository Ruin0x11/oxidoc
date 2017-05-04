use std;
use std::env;
use std::path::{Path, PathBuf};
use std::fs::remove_dir_all;

use store::Store;
use syntax::ast;
use syntax::diagnostics::plugin::DiagnosticBuilder;
use syntax::parse::{self, ParseSess};

use paths;
use document::*;
use convert::{Convert, Context};
use store::{self, Docset};
use toml_util;
use visitor::OxidocVisitor;

use ::errors::*;

fn parse<'a, T: ?Sized + AsRef<Path>>(path: &T,
                                      parse_session: &'a ParseSess)
                                      -> std::result::Result<ast::Crate, Option<DiagnosticBuilder<'a>>> {
    let path = path.as_ref();

    match parse::parse_crate_from_file(path, parse_session) {
        // There may be parse errors that the parser recovered from, which we
        // want to treat as an error.
        Ok(_) if parse_session.span_diagnostic.has_errors() => Err(None),
        Ok(krate) => Ok(krate),
        Err(e) => Err(Some(e)),
    }
}

pub fn generate_all() -> Result<()> {
    println!("Regenerating all documentation.");

    let home_dir: PathBuf;
    if let Some(x) = env::home_dir() {
        home_dir = x
    } else {
        bail!("Could not locate home directory");
    }

    let path = home_dir.as_path().join(".cargo/registry/doc");

    remove_dir_all(path);

    for src_dir in paths::src_iter(true, true)
        .chain_err(|| "Could not iterate cargo registry src directories")?
    {
        cache_doc_for_crate(&src_dir).
            chain_err(|| format!("Unable to generate documentation \
                                  for directory {}",
                                 &src_dir.display()))?;
    }
    Ok(())
}


pub fn generate(src_dir: PathBuf) -> Result<()> {
    cache_doc_for_crate(&src_dir).
        chain_err(|| format!("Unable to generate documentation \
                              for directory {}",
                             &src_dir.display()))?;

    Ok(())
}

fn get_crate_info(crate_path: &PathBuf) -> Result<CrateInfo> {
    let toml_path = crate_path.join("Cargo.toml");

    let toml_table = toml_util::toml_value_from_file(toml_path)?;

    let info = CrateInfo {
        name: toml_util::get_toml_value(&toml_table, "package", "name")?,
        version: toml_util::get_toml_value(&toml_table, "package", "version")?,
    };

    Ok(info)
}

/// Generates cached Rustdoc information for the given crate.
/// Expects the crate root directory as an argument.
fn cache_doc_for_crate(crate_path: &PathBuf) -> Result<()> {
    let info = get_crate_info(crate_path)?;

    println!("Generating documentation for {}", &info);

    let parse_session = ParseSess::new();

    // TODO: This has to handle [lib] targets and multiple [[bin]] targets.
    let mut main_path = crate_path.join("src/lib.rs");
    if !main_path.exists() {
        main_path = crate_path.join("src/main.rs");
        if !main_path.exists() {
            // TODO: Look for [lib] / [[bin]] targets here
            println!("No crate entry point found \
                      (nonstandard paths are unsupported)");
            return Ok(())
        }
    }
    let krate = parse(main_path.as_path(), &parse_session).unwrap();

    let mut store = generate_doc_cache(krate, info)
        .chain_err(|| "Failed to generate doc cache")?;

    store.save()
        .chain_err(|| "Couldn't save oxidoc data for module")
}

/// Generates documentation for the given crate.
fn generate_doc_cache(krate: ast::Crate, crate_info: CrateInfo) -> Result<Store> {
    let crate_doc_path = store::get_crate_doc_path(&crate_info)
        .chain_err(|| format!("Unable to get crate doc path for crate: {}",
                              &crate_info.name))?;

    let documents = {
        let mut v = OxidocVisitor::new(crate_info.clone());
        v.visit_crate(krate);
        let context = Context::new(crate_doc_path.clone(),
                                   crate_info.clone(),
                                   v.impls_for_ty.clone());
        v.convert(&context)
    };

    println!("Documents: {}", documents.len());

    for doc in &documents {
        doc.save()?;
    }

    let mut docset = Docset::new();
    docset.add_docs(documents)?;

    let mut store = Store::load();
    store.add_docset(crate_info, docset);
    store.save()?;

    Ok(store)
}
