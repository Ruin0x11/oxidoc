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

fn parse_name<'a>(name: &String) -> FnSig {
    let segs = ModPath::from(name.clone());
    if segs.0.len() == 1 {
        FnSig {
            scope: None,
            method: segs.0[0].identifier.clone(),
        }
    } else {
        FnSig {
            scope: Some(segs.parent()),
            method: segs.0.iter().last().unwrap().identifier.clone(),
        }
    }
}

fn render_method(fn_doc: &FnDoc) -> Result<()> {
    Ok(())
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
    stores_with_module: HashMap<ModPath, Vec<usize>>
}

impl Driver {
    pub fn new() -> Driver {
        let mut stores = Vec::new();
        println!("Making new driver");
        for path in paths::doc_iter(true, true).unwrap() {
            println!("Store at {}", &path.display());
            let mut store = Store::new(path).unwrap();
            store.load_cache();
            stores.push(store);
        }

        let mut stores_with_module = HashMap::new();
        for (i, store) in stores.iter().enumerate() {
            println!("store {}", i);
            let modules = store.get_modules();
            for m in modules {
                if !stores_with_module.contains_key(m) {
                    stores_with_module.insert(m.clone(), Vec::new());
                }
                println!("Store {} has module {}", store.path.display(), m);
                // v.push(i);
                // stores_with_module.insert(m, v);
            }
        }

        Driver {
            stores: stores,
            stores_with_module: stores_with_module,
        }
    }

    pub fn display_names(&self, names: Vec<String>) -> Result<()> {
        for name in names {
            let fn_sig = expand_name(&name)
                .chain_err(|| "Failure to display name")?;
            println!("name: {:?}", fn_sig);

            // self.display_name(name_exp)
            //     .chain_err(|| "No name to display");
        }
        Ok(())
    }

    fn display_name(&self, name: String) -> Result<()> {
        // TODO: Functions only.
        if let Err(_) = self.display_method(name) {
            bail!({"No method found"})
        }

        Ok(())
    }

    fn display_method(&self, name: String) -> Result<()> {
        // TODO: Attempt to filter here for a single match
        // If no match, list functions that have similar names
        let fn_docs = self.load_methods_matching(&name).
            chain_err(|| "No methods match")?;

        println!("= {}", &name);

        for fn_doc in fn_docs {
            // TODO: document construction should happen
            println!("(from crate {})", fn_doc.crate_info);
            println!("--------------------");
            render_method(&fn_doc);
            println!("--------------------");
        }
        Ok(())
    }

    //fn lookup_method(&self, name: String) -> Result<()>

    fn load_methods_matching(&self, name: &String) -> Result<Vec<FnDoc>> {
        let mut found = Vec::new();
        println!("attempting {}", &name);
        for loc in self.stores_containing(name).unwrap() {
            match loc.store.load_method(loc) {
                Ok(method) => {
                    println!("Found the method {} looking for {}", &method, &name);
                    found.push(method);
                }
                Err(mes) => println!("{:?}", mes)
            }
        }
        Ok(found)
    }

    fn stores_containing(&self, name: &String) -> Result<Vec<StoreLoc>> {
        let fn_sig = parse_name(name);

        let mut stores = Vec::new();
        let mut results = Vec::new();

        match fn_sig.scope {
            None => {
                // user gave name without path, look through all crate folders and their modules
                let mut v: Vec<usize> = Vec::new();
                for i in 0..self.stores.len() {
                    stores.push(i);
                }

                for idx in stores {
                    let store = self.stores.get(idx).unwrap();
                    for scope in store.get_modules() {
                        results.push(StoreLoc{
                            store: store,
                            scope: scope.clone(),
                            method: fn_sig.method.clone()
                        });
                    }
                }
            },
            Some(scope) => {
                // fully qualified scope was found, look in that crate's dir only
                // TODO: Could match against partial scope instead of exact.
                let stores = self.stores_with_module.get(&scope).unwrap();
                if !stores.len() == 0 {
                    let idx = stores.get(0).unwrap();
                    let store = self.stores.get(*idx).unwrap();
                    results.push(StoreLoc{
                        store: store,
                        scope: scope.clone(),
                        method: fn_sig.method.clone()
                    });
                }
            }
        };

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
