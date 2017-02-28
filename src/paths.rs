use std::iter::Iterator;
use std::env;
use std::fs;
use std::path::{PathBuf};
use ::errors::*;

pub fn src_iter(system: bool, cargo: bool) -> Result<Vec<PathBuf>> {
    let mut paths = Vec::new();

    if system {
        // TODO: push system .rd directory
    }

    if cargo {
        let cargo_src_path: PathBuf;
        if let Some(x) = env::home_dir() {
            cargo_src_path = x.join(".cargo/registry/src");
        } else {
            bail!("Could not get home directory");
        }

        let mut repo_paths = fs::read_dir(cargo_src_path.as_path())
            .chain_err(|| "Couldn't read cargo source path")?;

        // TODO: unsure what format the github-xxxx directories follow.
        let first = repo_paths.next().unwrap().unwrap();
        let meta = first.metadata();
        assert!(meta.is_ok());
        assert!(meta.unwrap().is_dir());
        let repo_path = first.path();

        let crate_src_paths = fs::read_dir(repo_path)
            .chain_err(|| "Couldn't read cargo repo path")?;

        for src in crate_src_paths {
            if let Ok(src_dir) = src {
                if let Ok(metadata) = src_dir.metadata() {
                    if metadata.is_dir() {
                        paths.push(src_dir.path());
                    }
                }
            }
        }
    }

    Ok(paths)
}

pub fn doc_iter(system: bool, cargo: bool) -> Result<Vec<PathBuf>> {
    let mut paths = Vec::new();

    if system {
        // TODO: push system .rd directory
    }

    if cargo {
        let cargo_doc_path: PathBuf;
        if let Some(x) = env::home_dir() {
            cargo_doc_path = x.join(".cargo/registry/doc");
        } else {
            bail!("Could not get home directory");
        }

        fs::create_dir_all(&cargo_doc_path.as_path())
            .chain_err(|| format!("Failed to create doc dir {}", &cargo_doc_path.display()))?;

        let doc_paths = fs::read_dir(cargo_doc_path.as_path())
            .chain_err(|| "Couldn't read cargo doc path")?;

        for doc in doc_paths {
            if let Ok(doc_dir) = doc {
                if let Ok(metadata) = doc_dir.metadata() {
                    if metadata.is_dir() {
                        paths.push(doc_dir.path());
                    }
                }
            }
        }
    }

    Ok(paths)
}
