use std::iter::Iterator;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use ::errors::*;

pub fn iter(system: bool, cargo: bool) -> Result<Vec<PathBuf>> {
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

        let mut crate_src_paths = fs::read_dir(repo_path)
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
