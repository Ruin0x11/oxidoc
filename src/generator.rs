use std;
use std::env;
use std::path::{Path, PathBuf};
use std::fs::{read_dir, remove_dir_all};

use store::Store;
use syntax::ast;
use syntax::diagnostics::plugin::DiagnosticBuilder;
use syntax::parse::{self, ParseSess};

use paths;
use document::*;
use convert::{Convert, Context, Documentation};
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

pub fn generate_all_docs() -> Result<()> {
    debug!("Regenerating all documentation.");
    generate_crate_registry_docs()?;

    if generate_stdlib_docs().is_err() {
        println!("The environment variable RUST_SRC_PATH was not set or malformed. Documentation \
                  for std won't be generated.");
    }

    Ok(())
}

pub fn generate_crate_registry_docs() -> Result<()> {
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
        generate_docs_for_path(src_dir)?;
    }
    Ok(())
}

pub fn generate_stdlib_docs() -> Result<()> {
    let rust_src_dir = env::var("RUST_SRC_PATH")
        .chain_err(|| format!("RUST_SRC_PATH was not set when trying to generate stdlib docs."))?;

    let stdlib_paths = read_dir(rust_src_dir)
        .chain_err(|| "Couldn't read rust source path")?;
    let mut paths = Vec::new();

    for src in stdlib_paths {
        if let Ok(src_dir) = src {
            if let Ok(metadata) = src_dir.metadata() {
                if metadata.is_dir() {
                    let mut path = src_dir.path();
                    path.push("Cargo.toml");
                    if path.exists() {
                        paths.push(src_dir.path());
                    }
                }
            }
        }
    }

    for path in paths {
        // BUG: ICE when attempting to parse rustdoc. Just skip parsing librustdoc.
        if !path.display().to_string().contains("librustdoc") {
            generate_docs_for_path(path)?;
        }
    }
    Ok(())
}

pub fn generate_docs_for_path(src_dir: PathBuf) -> Result<()> {
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
        lib_path: toml_util::get_toml_value(&toml_table, "lib", "path").ok(),
    };

    Ok(info)
}

/// Generates cached Rustdoc information for the given crate.
/// Expects the crate root directory as an argument.
fn cache_doc_for_crate(crate_path: &PathBuf) -> Result<()> {
    let info = get_crate_info(crate_path)?;

    println!("Generating documentation for {}", &info);

    let krate = match parse_crate(crate_path, &info) {
        Ok(k) => k,
        Err(_) => {
            println!("No crate entry point found \
                      (nonstandard paths are unsupported)");
            return Ok(())
        }
    };

    let mut store = generate_doc_cache(krate, info)
        .chain_err(|| "Failed to generate doc cache")?;

    store.save()
        .chain_err(|| "Couldn't save oxidoc data for module")
}

fn parse_crate(crate_path: &PathBuf, crate_info: &CrateInfo) -> Result<ast::Crate> {
    let parse_session = ParseSess::new();
    let lib_path = crate_info.lib_path.clone().unwrap_or("src/lib.rs".to_string());

    // TODO: This has to handle multiple [[bin]] targets.
    let mut main_path = crate_path.join(&lib_path);
    if !main_path.exists() {
        main_path = crate_path.join("src/main.rs");
        if !main_path.exists() {
            // TODO: Look for [[bin]] targets here
            bail!("No crate entry found");
        }
    }

    let krate = match parse(main_path.as_path(), &parse_session) {
        Ok(k) => k,
        Err(e) => bail!("Failed to parse crate {}: {:?}", crate_info.name, e),
    };

    Ok(krate)
}

pub fn generate_crate_docs(krate: ast::Crate, crate_info: CrateInfo) -> Result<Vec<Documentation>> {
    let crate_doc_path = store::get_crate_doc_path(&crate_info)
        .chain_err(|| format!("Unable to get crate doc path for crate: {}",
                              &crate_info.name))?;

    let mut v = OxidocVisitor::new(crate_info.clone());
    v.visit_crate(krate);
    let context = Context::new(crate_doc_path.clone(),
                               crate_info,
                               v.impls_for_ty.clone());
    Ok(v.convert(&context))
}

pub fn make_docset(documents: Vec<Documentation>) -> Result<Docset> {
    for doc in &documents {
        debug!("p: {}", doc.mod_path);
        doc.save()?;
    }

    let mut docset = Docset::new();
    docset.add_docs(documents)?;

    Ok(docset)
}

/// Generates documentation for the given crate.
pub fn generate_doc_cache(krate: ast::Crate, crate_info: CrateInfo) -> Result<Store> {
    let documents = generate_crate_docs(krate, crate_info.clone())?;
    let docset = make_docset(documents)?;

    let mut store = Store::load();
    store.add_docset(crate_info, docset);
    store.save()?;

    Ok(store)
}
