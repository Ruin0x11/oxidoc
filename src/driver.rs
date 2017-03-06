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

fn expand_name(name: &String) -> Result<DocSig> {
    let segs = ModPath::from(name.clone());
    let fn_sig = if segs.0.len() == 1 {
        DocSig {
            scope: None,
            identifier: segs.name().unwrap().identifier,
        }
    } else {
        DocSig {
            scope: Some(segs.parent().unwrap()),
            identifier: segs.name().unwrap().identifier 
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

    /// Takes a name, determines what kind of documentation it is referring to, and displays it.
    fn display_name(&self, name: &DocSig) -> Result<()> {
        if let Ok(x) = self.display_module(name) {
            return Ok(x)
        }

        if let Ok(x) = self.display_struct(name) {
            return Ok(x)
        }

        if let Ok(x) = self.display_function(name) {
            return Ok(x)
        }

        bail!(ErrorKind::NoDocumentationFound)
    }

    /// Attempts to find a module named 'name' in the oxidoc stores and print its documentation.
    fn display_module(&self, name: &DocSig) -> Result<()> {
        // TODO: Attempt to filter here for a single match
        // If no match, list functions that have similar names
        let module_docs = self.load_modules_matching(&name).
            chain_err(|| format!("No structs match the given name {}", &name))?;

        println!("= {}", &name);

        for module_doc in module_docs {
            // TODO: document construction should happen
            println!("{}", &module_doc);
        }
        Ok(())
    }

    /// Attempts to find a struct named 'name' in the oxidoc stores and print its documentation.
    fn display_struct(&self, name: &DocSig) -> Result<()> {
        // TODO: Attempt to filter here for a single match
        // If no match, list structs that have similar names
        let struct_docs = self.load_structs_matching(&name).
            chain_err(|| format!("No structs match the given name {}", &name))?;

        println!("= {}", &name);

        for struct_doc in struct_docs {
            // TODO: document construction should happen
            println!("{}", &struct_doc);
        }
        Ok(())
    }

    /// Attempts to find a fn named 'name' in the oxidoc stores and print its documentation.
    fn display_function(&self, name: &DocSig) -> Result<()> {
        // TODO: Attempt to filter here for a single match
        // If no match, list functions that have similar names
        let fn_docs = self.load_functions_matching(&name).
            chain_err(|| format!("No functions match the given name {}", &name))?;

        println!("= {}", &name);

        for fn_doc in fn_docs {
            // TODO: document construction should happen
            println!("{}", &fn_doc);
        }
        Ok(())
    }

    /// Obtains documentation for structs with the identifier 'name'
    fn load_modules_matching(&self, name: &DocSig) -> Result<Vec<ModuleDoc>> {
        let mut found = Vec::new();
        for loc in self.stores_containing(name).unwrap() {
            if let Ok(module) = loc.store.load_module(&loc.scope) {
                info!("Found the module {} looking for {}", &module, &name);
                found.push(module);
            }
        }
        if found.len() == 0 {
            bail!("No modules matched name {}", name);
        }
        Ok(found)
    }

    /// Obtains documentation for structs with the identifier 'name'
    fn load_structs_matching(&self, name: &DocSig) -> Result<Vec<StructDoc>> {
        let mut found = Vec::new();
        for loc in self.stores_containing(name).unwrap() {
            if let Ok(strukt) = loc.store.load_struct(&loc.scope, &loc.identifier) {
                info!("Found the struct {} looking for {}", &strukt, &name);
                found.push(strukt);
            }
        }
        if found.len() == 0 {
            bail!("No structs matched name {}", name);
        }
        Ok(found)
    }

    /// Obtains documentation for functions with the signature 'name'
    fn load_functions_matching(&self, name: &DocSig) -> Result<Vec<FnDoc>> {
        let mut found = Vec::new();
        for loc in self.stores_containing(name).unwrap() {
            if let Ok(function) = loc.store.load_function(&loc.scope, &loc.identifier) {
                info!("Found the function {} looking for {}", &function, &name);
                found.push(function);
            }
        }
        if found.len() == 0 {
            bail!("No functions matched name {}", name);
        }
        Ok(found)
    }

    /// Obtains a list of oxidoc stores the given documentation identifier could possibly exist in.
    fn stores_containing(&self, sig: &DocSig) -> Result<Vec<StoreLoc>> {
        let mut stores = Vec::new();
        let mut results = Vec::new();

        info!("looking for store that has {}", &sig);

        match sig.scope {
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
                            scope: scope.clone(),
                            identifier: sig.identifier.clone()
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
                            identifier: sig.identifier.clone()
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

    #[test]
    fn test_expand_name() {
        let s = &"root::a::b".to_string();
        assert_eq!(expand_name(s).unwrap(), DocSig{
            scope: Some(ModPath::from("root::a".to_string())),
            identifier: "b".to_string()
        });

        let s = &"run".to_string();
        assert_eq!(expand_name(s).unwrap(), DocSig{
            scope: None,
            identifier: "run".to_string()
        });
    }
}
