use serde::ser::{Serialize};
use serde::de::{Deserialize};
use paths;
use store::*;
use ::errors::*;
use std::collections::HashMap;
use std::fmt;
use std::fmt::Display;
use document::*;

error_chain! {
    errors {
        NoDocumentationFound {
            description("No documentation could be found.")
        }
    }
}

fn expand_name(name: &String) -> Result<ModPath> {
    let segs = ModPath::from(name.clone());
    Ok(segs)
}

pub struct Driver {
    stores: Vec<Store>,
    store_with_module: HashMap<ModPath, usize>
}

impl Driver {
    pub fn new() -> Result<Driver> {
        let mut stores = Vec::new();
        for path in paths::doc_iter(true, true).unwrap() {
            info!("Found store at {}", &path.display());
            let mut store = Store::new(path).unwrap();
            store.load_cache()
                .chain_err(|| "Failed to load store cache")?;
            stores.push(store);
        }

        let mut store_with_module = HashMap::new();
        for (i, store) in stores.iter().enumerate() {
            let modules = store.get_modpaths();
            for m in modules {
                store_with_module.insert(m.clone(), i);
            }
        }

        Ok(Driver {
            stores: stores,
            store_with_module: store_with_module,
        })
    }

    /// Takes a list of name queries and searches for documentation for each.
    pub fn display_names(&self, names: Vec<String>) -> Result<()> {
        for name in names {
            let fn_sig = expand_name(&name)
                .chain_err(|| "Failure to display name")?;
            info!("name: {:?}", fn_sig);

            self.display_name(&fn_sig)
                .chain_err(|| format!("Could not resolve {} to documentation", fn_sig))?;
        }
        Ok(())
    }

    /// Takes a module path, determines what kind of documentation it is referring to, and displays it.
    fn display_name(&self, name: &ModPath) -> Result<()> {
        // TODO: Currently looking for everything blindly.
        if let Ok(x) = self.display_doc::<ModuleDoc_>(name) {
            return Ok(x)
        }

        if let Ok(x) = self.display_doc::<StructDoc_>(name) {
            return Ok(x)
        }

        if let Ok(x) = self.display_doc::<FnDoc_>(name) {
            return Ok(x)
        }

        bail!(ErrorKind::NoDocumentationFound)
    }

    /// Attempts to find a fn named 'name' in the oxidoc stores and print its documentation.
    fn display_doc<T: Documentable + Serialize + Deserialize>(&self, name: &ModPath) -> Result<()> {
        // TODO: Attempt to filter here for a single match
        // If no match, list functions that have similar names
        let docs = self.load_docs_matching::<T>(&name).
            chain_err(|| format!("No documents match the given name {}", &name))?;

        println!("= {}", &name);

        for doc in docs {
            // TODO: document construction should happen
            println!("{}", &doc);
        }
        Ok(())
    }

    fn load_docs_matching<T: Documentable + Serialize + Deserialize>(&self, name: &ModPath) -> Result<Vec<Document<T>>> {
        let mut found = Vec::new();
        for loc in self.stores_containing(name).unwrap() {
            let full_path = ModPath::join(&loc.path, name);
            if let Ok(module) = loc.store.load_doc::<T>(&full_path) {
                info!("Found the documentation {} looking for {}", &module, &name);
                found.push(module);
            }
        }
        if found.len() == 0 {
            bail!("No modules matched name {}", name);
        }
        Ok(found)
        
    }

    /// Obtains a list of oxidoc stores the given documentation identifier could possibly exist in.
    fn stores_containing(&self, path: &ModPath) -> Result<Vec<StoreLoc>> {
        let mut stores = Vec::new();
        let mut results = Vec::new();

        info!("looking for store that has {}", &path);

        let parent = path.parent();
        match parent {
            None => {
                info!("Looking through everything");
                // user gave name without path, look through all crate folders and their modules
                for i in 0..self.stores.len() {
                    stores.push(i);
                }

                for idx in stores {
                    let store = self.stores.get(idx).unwrap();
                    info!("Store: {:?}", &store);
                    for scope in store.get_modpaths() {
                        results.push(StoreLoc{
                            store: store,
                            path: scope.clone(),
                        });
                    }
                }
            },
            Some(ref scope) => {
                // fully qualified scope was found, look in that crate's dir only
                // TODO: Could match against partial scope instead of exact.
                info!("Looking at {}", scope);
                let stores_w = self.store_with_module.get(&scope);
                match stores_w {
                    Some(idx) => {
                        let store = self.stores.get(*idx).unwrap();
                        info!("Store found! {}", store.path.display());
                        results.push(StoreLoc{
                            store: store,
                            path: path.clone(),
                        });
                    }
                    None => {
                        info!("No modules found, given {}", &scope);
                        // No modules with the given path were found
                        return Ok(Vec::new());
                    }
                }
            }
        };

        Ok(results)
    }
}

#[cfg(test)]
mod test {
    use super::*;
}
