use std;
use store::Store;
use toml;

use std::collections::HashMap;
use std::fmt::{self, Display};
use std::env;
use std::path::{Path, PathBuf};
use std::io::{Read};
use std::fs::{File, remove_dir_all};

use syntax::abi;
use syntax::ast::{self, ViewPath};
use syntax::attr;
use syntax::print::pprust;
use syntax::codemap::Spanned;
use syntax::codemap::{Span};
use syntax::diagnostics::plugin::DiagnosticBuilder;
use syntax::parse::{self, ParseSess};
use syntax::visit::{self, Visitor};
use syntax::symbol::{Symbol};

use paths;
use document::*;

use errors::*;

use visitor::*;

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

pub fn generate_all() -> Result<()> {
    println!("Regenerating all documentation.");

    let home_dir: PathBuf;
    if let Some(x) = env::home_dir() {
        home_dir = x
    } else {
        bail!("Could not locate home directory");
    }

    let path = home_dir.as_path().join(".cargo/registry/doc");

    remove_dir_all(path)
        .chain_err(|| "Could not remove cargo doc directory")?;

    for src_dir in paths::src_iter(true, true)
        .chain_err(|| "Could not iterate cargo registry src directories")?
    {
        cache_doc_for_crate(&src_dir).
            chain_err(|| format!("Unable to generate documentation for directory {}", &src_dir.display()))?;
    }
    Ok(())
}


pub fn generate(src_dir: PathBuf) -> Result<()> {
    cache_doc_for_crate(&src_dir).
        chain_err(|| format!("Unable to generate documentation for directory {}", &src_dir.display()))?;

    Ok(())
}

/// Generates cached Rustdoc information for the given crate.
/// Expects the crate root directory as an argument.
fn cache_doc_for_crate(crate_path: &PathBuf) -> Result<()> {
    let toml_path = crate_path.join("Cargo.toml");

    let mut fp = File::open(&toml_path).chain_err(|| format!("Could not find Cargo.toml in path {}", toml_path.display()))?;

    let ref mut contents = String::new();
    fp.read_to_string(contents).chain_err(|| "Failed to read from file")?;

    let info: CrateInfo = toml::de::from_str(contents).chain_err(|| "Couldn't parse Cargo.toml")?;

    println!("Generating documentation for {}", &info);

    let parse_session = ParseSess::new();

    // TODO: This has to handle [lib] targets and multiple [[bin]] targets.
    let mut main_path = crate_path.join("src/lib.rs");
    if !main_path.exists() {
        main_path = crate_path.join("src/main.rs");
        if!main_path.exists() {
            // TODO: Look for [lib] / [[bin]] targets here
            println!("No crate entry point found (nonstandard paths are unsupported)");
            return Ok(())
        }
    }
    let krate = parse(main_path.as_path(), &parse_session).unwrap();

    let store = generate_doc_cache(&krate, info)
        .chain_err(|| "Failed to generate doc cache")?;

    // TODO: save all to disk once, not as we go
    store.save()
        .chain_err(|| "Couldn't save oxidoc data for module")
}

/// Obtains the base output path for a crate's documentation.
fn get_crate_doc_path(crate_info: &CrateInfo) -> Result<PathBuf> {
    let home_dir: PathBuf;
    if let Some(x) = env::home_dir() {
        home_dir = x
    } else {
        bail!("Could not locate home directory");
    }

    let path = home_dir.as_path().join(".cargo/registry/doc")
        .join(format!("{}-{}", crate_info.package.name, crate_info.package.version));
    Ok(path)
}

/// Generates documentation for the given crate.
fn generate_doc_cache(krate: &ast::Crate, crate_info: CrateInfo) -> Result<Store> {

    let crate_doc_path = get_crate_doc_path(&crate_info)
        .chain_err(|| format!("Unable to get crate doc path for crate: {}", &crate_info.package.name))?;

    // Clear out old doc path
    if crate_doc_path.exists() {
        remove_dir_all(&crate_doc_path);
    }

    let mut visitor = RustdocVisitor {
        store: Store::new(crate_doc_path).unwrap(),
        current_scope: ModPath(Vec::new()),
        crate_info: crate_info.clone(),
        items: Vec::new(),
        docstrings: Vec::new(),
        used_namespaces: Vec::new(),
        is_part_of_use: false,
    };

    // Push the crate name onto the current namespace so
    // the module "module" will resolve to "crate::module"
    visitor.current_scope.push(PathSegment{
        identifier: crate_info.package.name.clone()
    });

    // Also add the crate's namespace as a known documentation path
    visitor.store.add_modpath(visitor.current_scope.clone());

    // And add the crate itself as documentation
    let doc = match get_doc(&krate.attrs) {
        Some(d) => {
            info!("Crate doc: {}", d);
            d
        },
        None    => "".to_string(),
    };

    visitor.docstrings.push(doc);

    // visitor.store.add_module(Document{
    //     crate_info: visitor.crate_info.clone(),
    //     path: visitor.current_scope.clone(),
    //     signature: format!("crate {}", visitor.crate_info.package.name),
    //     docstring: doc,
    //     doc: ModuleDoc {
    //         fn_docs: Vec::new(),
    //         struct_docs: Vec::new(),
    //         module_docs: Vec::new(),
    //     }
    // });

    // Create a list of 'use'd namespaces for the crate's namespace
    visitor.used_namespaces.push(HashMap::new());

    visitor.visit_mod(&krate.module, krate.span, ast::CRATE_NODE_ID);

    Ok(visitor.store)
}

#[cfg(test)]
mod test {
    use super::*;
    use env_logger;

    fn test_harness(s: &str) -> Result<Store> {
        let parse_session = ParseSess::new();
        let krate = match parse::parse_crate_from_source_str("test.rs".to_string(), s.to_string(), &parse_session) {
            Ok(_) if parse_session.span_diagnostic.has_errors() => bail!("Parse error"),
            Ok(krate) => krate,
            Err(_) => bail!("Failed to parse"),
        };

        let crate_info = CrateInfo {
            package: Package {
                name: "test".to_string(),
                version: "1.0.0".to_string(),
            }
        };

        generate_doc_cache(&krate, crate_info)
    }


    #[test]
    fn test_has_modules() {
        let _ = env_logger::init();
        let store = test_harness(r#"
        mod a {
            mod b {
            }
        }"#).unwrap();
        let modules = store.get_modpaths();
        let p = &ModPath::from("test".to_string());
        assert!(modules.contains(p));
        let p = &ModPath::from("test::a".to_string());
        assert!(modules.contains(p));
        let p = &ModPath::from("test::a::b".to_string());
        assert!(modules.contains(p));
    }

    #[test]
    fn test_module_has_fns() {
        let _ = env_logger::init();
        let store = test_harness(r#"
        fn main() {
          println!("inside main");
        }
        mod a {
            mod b {
                fn thing() {
                    println!("Hello, world!");
                }
                struct Mine(u32);
                impl Mine {
                    fn print_val(&self) {
                        println!("{}", self.0);
                    }
                }
            }
            impl b::Mine {
                fn print_val_plus_two(&self) {
                    println!("{}", self.0 + 2);
                }
            }
        }"#).unwrap();
        let functions = store.get_functions(&ModPath::from("test::a::b".to_string())).unwrap();
        let f = &"thing".to_string();
        assert!(functions.contains(f));
        let functions = store.get_functions(&ModPath::from("test".to_string())).unwrap();
        let f = &"main".to_string();
        assert!(functions.contains(f));
        let functions = store.get_functions(&ModPath::from("test::a::b::Mine".to_string())).unwrap();
        let f = &"print_val".to_string();
        assert!(functions.contains(f));
        let f = &"print_val_plus_two".to_string();
        assert!(functions.contains(f));
    }

    #[test]
    fn test_get_doc_fn() {
        let _ = env_logger::init();
        let store = test_harness(r#"
        fn main() {
          println!("inside main");
        }
        mod a {
            mod b {
                /// Prints a message.
                /// Sort of useful.
                fn thing() {
                    println!("Hello, world!");
                }
                pub struct Mine(pub u32);
                impl Mine {
                    /// Prints this struct's value.
                    /// Mildly useful.
                    fn print_val(&self) {
                        println!("{}", self.0);
                    }
                }
            }
            impl b::Mine {
                /// Prints this struct's value plus 2.
                /// Somewhat useful.
                fn print_val_plus_two(&self) {
                    println!("{}", self.0 + 2);
                }
            }
        }"#).unwrap();
        store.save().unwrap();
        let function = store.load_doc::<FnDoc>(&ModPath::from("test::a::b::thing".to_string())).unwrap();

        assert_eq!(function.signature, "fn thing()".to_string());
        assert_eq!(function.docstring, "/// Prints a message.\n/// Sort of useful.".to_string());
        let function = store.load_doc::<FnDoc>(&ModPath::from("test::a::b::Mine::print_val".to_string())).unwrap();
        assert_eq!(function.signature, "fn print_val(&self)".to_string());
        assert_eq!(function.docstring, "/// Prints this struct's value.\n/// Mildly useful.".to_string());
        let function = store.load_doc::<FnDoc>(&ModPath::from("test::a::b::Mine::print_val_plus_two".to_string())).unwrap();
        assert_eq!(function.signature, "fn print_val_plus_two(&self)".to_string());
        assert_eq!(function.docstring, "/// Prints this struct's value plus 2.\n/// Somewhat useful.".to_string());
    }

    #[test]
    fn test_get_doc_struct() {
        let _ = env_logger::init();
        let store = test_harness(r#"
        //! Crate documentation.

        struct UndoccedStruct;

        /// Documentation for MyStruct.
        /// It is nice.
        struct MyStruct;
        "#).unwrap();
        store.save().unwrap();
        let strukt = store.load_doc::<StructDoc>(&ModPath::from("test::UndoccedStruct".to_string())).unwrap();
        assert_eq!(strukt.docstring, "".to_string());
        let strukt = store.load_doc::<StructDoc>(&ModPath::from("test::MyStruct".to_string())).unwrap();
        assert_eq!(strukt.docstring, "/// Documentation for MyStruct.\n/// It is nice.".to_string());
    }

    #[test]
    fn test_get_doc_module() {
        let _ = env_logger::init();
        let store = test_harness(r#"
        //! Crate documentation.
        //! A test crate.

        mod a {
          //! module a
          //! is a module
        }
        "#).unwrap();
        store.save().unwrap();
        let module = store.load_doc::<ModuleDoc>(&ModPath::from("test".to_string())).unwrap();
        assert_eq!(module.docstring, "//! Crate documentation.\n//! A test crate.".to_string());
        let module = store.load_doc::<ModuleDoc>(&ModPath::from("test::a".to_string())).unwrap();
        assert_eq!(module.docstring, "//! module a\n//! is a module".to_string());
    }


    #[test]
    fn test_get_doc_use() {
        let _ = env_logger::init();
        let store = test_harness(r#"
        mod b {
            mod a {
            struct St(i32);
            }
            use a::St;
        }
        impl b::St {
          fn print_val_plus_two(&self) {
            println!("{}", self.0 + 2);
          }
        }
        use b::St;
        impl St {
          fn print_val(&self) {
            println!("{}", self.0);
          }
        }
        fn main() {
          let s = St(10);
          s.print_val();
          s.print_val_plus_two();
        }
        "#).unwrap();
        let functions = store.get_functions(&ModPath::from("test::b::a::St".to_string())).unwrap();
        let f = &"print_val".to_string();
        assert!(functions.contains(f));
        let f = &"print_val_plus_two".to_string();
        assert!(functions.contains(f));
    }

    /// github issue #3
    #[test]
    fn test_use_super() {
        let _ = env_logger::init();
        let store = test_harness(r#"
        pub mod a {
            pub struct MyStruct;

            pub mod b {
                use super::MyStruct;

                impl MyStruct {
                    // Documentation for test_a.
                    pub fn test_a(&self) {
                        println!("Hello, world!");
                    }
                }
            }
        }

        fn main() {
            let test = a::MyStruct;
            test.test_a();

        }
        "#).unwrap();
        let functions = store.get_functions(&ModPath::from("test::a::MyStruct".to_string())).unwrap();
        let f = &"test_a".to_string();
        assert!(functions.contains(f));
    }

    /// github issue #2
    #[test]
    fn test_use_globbed() {
        let _ = env_logger::init();
        let store = test_harness(r#"
        pub mod a {
            pub struct MyStruct;
        }

        pub mod b {
            use a::*;

            impl MyStruct {
                /// Documentation for test_a.
                pub fn test_a(&self) {
                    println!("Hello, world!");
                }
            }
        }

        fn main() {
            let one = a::MyStruct;
            one.test_a();
        }
        "#).unwrap();
        let functions = store.get_functions(&ModPath::from("test::a::MyStruct".to_string())).unwrap();
        let f = &"test_a".to_string();
        assert!(functions.contains(f));
    }
}
