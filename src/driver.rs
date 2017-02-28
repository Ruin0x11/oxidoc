use paths;
use store::*;
use generator::{FnDoc, ModPath};
use ::errors::*;
use std::collections::HashMap;


struct FnSig {
    pub scope: ModPath,
    // TODO: selector
    pub method: String,
}


fn parse_name<'a>(name: &String) -> FnSig {
    let segs = ModPath::from(name.clone());
    if segs.0.len() == 1 {
        FnSig {
            scope: ModPath(Vec::new()),
            method: segs.0[0].identifier.clone(),
        }
    } else {
        FnSig {
            scope: segs.parent(),
            method: segs.0.iter().last().unwrap().identifier.clone(),
        }
    }
}

fn render_method() -> Result<()> {
    Ok(())
}

pub struct Driver {
    stores: Vec<Store>,
    stores_with_module: HashMap<String, Vec<usize>>
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
            let name_exp = self.expand_name(name)
                .chain_err(|| "Failure to display name")?;
            println!("Name: {}", name_exp);
        }
        Ok(())
    }

    fn expand_name(&self, name: String) -> Result<String> {
        let candidates = self.load_methods_matching(name)?;

        // TODO: Detect if a type prefix is used (:fn)

        // path_segs.iter().fold(String::new(), |res, s| res + &s.identifier)
        let result = candidates.iter().fold(String::new(), |res, c| res + "\n" + &c.signature.clone() );
        Ok(result)
    }

    fn display_name(&self, name: String) -> Result<()> {
        // TODO: Functions only.
        if let Err(_) = self.display_method(name) {
            bail!({"No method found"})
        }

        Ok(())
    }

    fn display_method(&self, name: String) -> Result<()> {
        // let mut out = Document::new();

        // out.add_method(name);

        // out.display();
        Ok(())
    }

    fn load_methods_matching(&self, name: String) -> Result<Vec<FnDoc>> {
        let mut found = Vec::new();
        for loc in self.stores_containing(&name).unwrap() {
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
        let ambiguous = fn_sig.scope.0.is_empty();

        let mut stores = Vec::new();
        if ambiguous {
            // look through all crate folders
            let mut v: Vec<usize> = Vec::new();
            for i in 0..self.stores.len() {
                stores.push(i);
            }
        } else {
            // scope was found, look in that crate's dir only
            stores.extend(self.stores_with_module.get(&fn_sig.scope.to_string()).unwrap());
        };

        let mut results = Vec::new();
        for store in stores {
            results.push(StoreLoc{ store: self.stores.get(store).unwrap(), scope: fn_sig.scope.clone(), method: fn_sig.method.clone() })
        }

        Ok(results)
    }
}
