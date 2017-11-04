use std::{env, fs, io, result};
use std::path::{PathBuf};
use document::CrateInfo;
use ::errors::*;

const STORE_FILENAME: &str = "store";

pub fn home_dir() -> Result<PathBuf> {
    if let Some(home_dir) = env::home_dir() {
        Ok(home_dir)
    } else {
        bail!(ErrorKind::NoHomeDirectory)
    }
}

fn make_registry_path(dir: &str) -> Result<PathBuf> {
    let home_dir = home_dir()?;

    Ok(home_dir.as_path().join(".cargo").join("registry").join(dir))
}

pub fn doc_registry_path() -> Result<PathBuf> {
    make_registry_path("doc")
}

pub fn src_registry_path() -> Result<PathBuf> {
    make_registry_path("src")
}

pub fn store_file_path() -> Result<PathBuf> {
    let mut registry_path = doc_registry_path()?;
    registry_path.push(STORE_FILENAME);
    Ok(registry_path)
}

/// Obtains the base output path for a crate's documentation.
pub fn crate_doc_path(crate_info: &CrateInfo) -> Result<PathBuf> {
    let registry_path = doc_registry_path()?;

    let path = registry_path.join(format!("{}-{}",
                                          crate_info.name,
                                          crate_info.version));
    Ok(path)
}

pub fn iter_crate_source_paths() -> Result<Vec<PathBuf>> {
    let mut paths = Vec::new();

    let cargo_src_path = src_registry_path()?;

    let mut repo_paths = fs::read_dir(cargo_src_path.as_path())
        .chain_err(|| "Couldn't read cargo source path")?;

    // TODO: unsure what format the github-xxxx directories follow.
    let first = repo_paths.next().unwrap().unwrap();
    let meta = first.metadata();
    if meta.is_err() || !meta.unwrap().is_dir() {
        bail!("Failed to read directory");
    }
    let repo_path = first.path();

    let crate_src_paths = fs::read_dir(repo_path)
        .chain_err(|| "Couldn't read cargo repo path")?;

    for src in crate_src_paths {
        if let Some(path) = validate_crate_src_path(src) {
            paths.push(path);
        }
    }

    Ok(paths)
}

fn validate_crate_src_path(src: result::Result<fs::DirEntry, io::Error>) -> Option<PathBuf> {
    if let Ok(src_dir) = src {
        if let Ok(metadata) = src_dir.metadata() {
            if metadata.is_dir() {
                return Some(src_dir.path());
            }
        }
    }

    None
}
