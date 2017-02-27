use std::env;
use std::fs;
use std::path::Path;

pub fn iter(system: bool, cargo: bool) -> IntoIter<Path> {
    let paths = Vec::new();

    if system {
        // TODO: push system .rd directory
    }

    if cargo {
        let cargo_src_path: String;
        if let Some(x) = env::home_dir() {
            cargo_src_path = x.join(".cargo/registry/src");
        } else {
            bail!("Could not get home directory");
        }

        let repo_paths = fs::read_dir(cargo_src_path)
            .chain_err("Couldn't read cargo source path")?;

        // TODO: unsure what format the github-xxxx directories follow.
        if !repo_paths.first().is_dir() {
            bail!("Didn't find expected source directory");;
        }

        let crate_src_paths = fs::read_dir(repo_paths.first())
            .chain_err("Couldn't read cargo repo path")?;

        for src in crate_src_paths {
            if src.is_dir() {
                paths.push_back(src);
            }
        }
    }

    paths.into_iter()
}
