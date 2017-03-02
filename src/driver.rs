use paths;
use store::*;
use generator::{FnDoc, ModPath};
use ::errors::*;
use std::collections::HashMap;
use std::fmt;
use std::fmt::Display;
use generator::PathSegment;

#[derive(Eq, PartialEq, Debug)]
struct FnSig {
    pub scope: Option<ModPath>,
    // TODO: selector
    pub method: String,
}

impl Display for FnSig {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut scope = match self.scope {
            Some(ref scope) => scope.clone(),
            None => ModPath(Vec::new())
        };
        scope.push(PathSegment{identifier: self.method.clone()});

        write!(f, "{}", scope.to_string())
    }
}

fn expand_name(name: &String) -> Result<FnSig> {
    let segs = ModPath::from(name.clone());
    let fn_sig = if segs.0.len() == 1 {
        FnSig {
            scope: None,
            method: segs.name().identifier,
        }
    } else {
        FnSig {
            scope: Some(segs.parent()),
            method: segs.name().identifier 
        }
    };
    Ok(fn_sig)
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
            let modules = store.get_modules();
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

    /// Takes a name, determines what kind of documentation it is referring to, and displays it.
    fn display_name(&self, name: &FnSig) -> Result<()> {
        // TODO: Only methods are currently supported.
        self.display_method(name)
            .chain_err(|| format!("No documentation found for {}", &name))
    }

    /// Attempts to find a fn named 'name' in the rd stores and print its documentation.
    fn display_method(&self, name: &FnSig) -> Result<()> {
        // TODO: Attempt to filter here for a single match
        // If no match, list functions that have similar names
        let fn_docs = self.load_methods_matching(&name).
            chain_err(|| format!("No methods match the given name {}", &name))?;

        println!("= {}", &name);

        for fn_doc in fn_docs {
            // TODO: document construction should happen
            println!("{}", &fn_doc);
        }
        Ok(())
    }

    /// O
    fn load_methods_matching(&self, name: &FnSig) -> Result<Vec<FnDoc>> {
        let mut found = Vec::new();
        for loc in self.stores_containing(name).unwrap() {
            if let Ok(method) = loc.store.load_method(loc) {
                info!("Found the method {} looking for {}", &method, &name);
                found.push(method);
            }
        }
        if found.len() == 0 {
            bail!("No methods matched name {}", name);
        }
        Ok(found)
    }

    /// Obtains a list of rd stores the method could possibly exist in.
    ///  
    fn stores_containing(&self, fn_sig: &FnSig) -> Result<Vec<StoreLoc>> {
        let mut stores = Vec::new();
        let mut results = Vec::new();

        info!("looking for store that has {}", &fn_sig);

        match fn_sig.scope {
            None => {
                info!("Looking through everything");
                // user gave name without path, look through all crate folders and their modules
                for i in 0..self.stores.len() {
                    stores.push(i);
                }

                for idx in stores {
                    let store = self.stores.get(idx).unwrap();
                    info!("Store: {:?}", &store);
                    for scope in store.get_modules() {
                        results.push(StoreLoc{
                            store: store,
                            scope: scope.clone(),
                            method: fn_sig.method.clone()
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
                            scope: scope.clone(),
                            method: fn_sig.method.clone()
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
mod tests {
    use super::*;

    #[test]
    fn test_expand_name() {
        let s = &"root::a::b".to_string();
        assert_eq!(expand_name(s).unwrap(), FnSig{
            scope: Some(ModPath::from("root::a".to_string())),
            method: "b".to_string()
        });

        let s = &"run".to_string();
        assert_eq!(expand_name(s).unwrap(), FnSig{
            scope: None,
            method: "run".to_string()
        });
    }
}
