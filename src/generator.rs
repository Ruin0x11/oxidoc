use std;
use store::Store;
use toml;

use std::fmt::{self, Display};
use std::env;
use std::path::{Path, PathBuf};
use std::io::{Read};
use std::fs::{File, remove_dir_all};

use syntax::ast;
use syntax::abi;
use syntax::print::pprust;
use syntax::codemap::Spanned;
use syntax::codemap::{Span};
use syntax::diagnostics::plugin::DiagnosticBuilder;
use syntax::parse::{self, ParseSess};
use syntax::visit::{self, FnKind, Visitor};

use paths;
use document::*;

use errors::*;

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

struct RustdocCacher<'v> {
    store: Store,
    current_scope: ModPath,
    crate_info: CrateInfo,
    items: Vec<&'v ast::Item>,
}

impl<'v> RustdocCacher<'v> {
    /// Pushes a segment onto the current path scope.
    pub fn push_path(&mut self, ident: ast::Ident) {
        let seg = PathSegment{ identifier: pprust::ident_to_string(ident) };
        self.current_scope.push(seg);
    }

    /// Pops a segment from the current path scope.
    pub fn pop_path(&mut self) {
        self.current_scope.pop();
    }

}

impl<'v> Visitor<'v> for RustdocCacher<'v> {
    //fn visit_fn(&mut self,
    //fk: FnKind<'ast>, fd: &'ast FnDecl, s: Span, _: NodeId) {
    fn visit_fn(&mut self,
                fn_kind: FnKind<'v>,
                fn_decl: &'v ast::FnDecl,
                //block: &'v ast::Block,
                span: Span,
                _id: ast::NodeId) {
        match fn_kind {
            FnKind::ItemFn(id, gen, unsafety, Spanned{ node: constness, span: _ }, abi, visibility, _) => {
                let sig = pprust::to_string(|s| s.print_fn(fn_decl, unsafety, constness,
                                                           abi, Some(id), gen, visibility));

                // convert ast types to our Serializable types.

                let my_unsafety = match unsafety {
                    ast::Unsafety::Normal => Unsafety::Normal,
                    ast::Unsafety::Unsafe => Unsafety::Unsafe,
                };

                let my_constness = match constness {
                    ast::Constness::Const    => Constness::Const,
                    ast::Constness::NotConst => Constness::NotConst,
                };

                let my_visibility = match *visibility {
                    ast::Visibility::Public => Visibility::Public,
                    _                       => Visibility::Private,
                };

                let my_abi = match abi {
                    abi::Abi::Cdecl             => Abi::Cdecl,
                    abi::Abi::Stdcall           => Abi::Stdcall,
                    abi::Abi::Fastcall          => Abi::Fastcall,
                    abi::Abi::Vectorcall        => Abi::Vectorcall,
                    abi::Abi::Aapcs             => Abi::Aapcs,
                    abi::Abi::Win64             => Abi::Win64,
                    abi::Abi::SysV64            => Abi::SysV64,
                    abi::Abi::PtxKernel         => Abi::PtxKernel,
                    abi::Abi::Msp430Interrupt   => Abi::Msp430Interrupt,
                    abi::Abi::Rust              => Abi::Rust,
                    abi::Abi::C                 => Abi::C,
                    abi::Abi::System            => Abi::System,
                    abi::Abi::RustIntrinsic     => Abi::RustIntrinsic,
                    abi::Abi::RustCall          => Abi::RustCall,
                    abi::Abi::PlatformIntrinsic => Abi::PlatformIntrinsic,
                    abi::Abi::Unadjusted        => Abi::Unadjusted
                };

                let my_path = ModPath::join(&self.current_scope,
                                            &ModPath::from_ident(span, id));
                let fn_doc = FnDoc {
                    crate_info: self.crate_info.clone(),
                    path: my_path,
                    signature: sig,
                    unsafety: my_unsafety,
                    constness: my_constness,
                    // TODO: Generics
                    visibility: my_visibility,
                    abi: my_abi,
                };

                self.store.add_function(fn_doc);
            },
            FnKind::Method(_, _, _, _) => {
                //TODO: This makes sense only in the context of an impl / Trait
                //id.name.as_str().to_string(),
            },
            FnKind::Closure(_) => () // Don't care.
        };

        // Continue walking the rest of the funciton so we pick up any functions
        // or closures defined in its body.
        visit::walk_fn(self, fn_kind, fn_decl, span);
    }

    fn visit_mac(&mut self, _mac: &'v ast::Mac) {
        // TODO: No, it isn't fine...
    }

    fn visit_variant_data(&mut self, var: &'v ast::VariantData, id: ast::Ident,
                          _: &'v ast::Generics, node_id: ast::NodeId, span: Span) {

        let my_path = ModPath::join(&self.current_scope,
                                    &ModPath::from_ident(span, id));
        let sig = format!("{} {} {{ /* fields omitted */ }}",
                          pprust::visibility_qualified(&self.items.iter().last().unwrap().vis,
                                                       &"struct"),
                          pprust::ident_to_string(id));

        let struct_doc = StructDoc {
            crate_info: self.crate_info.clone(),
            path: my_path,
            signature: sig,
            fn_docs: Vec::new(),
        };

        self.store.add_struct(struct_doc);

        visit::walk_struct_def(self, var);
    }

    fn visit_item(&mut self, item: &'v ast::Item) {
        // Keep track of the path we're in as we traverse modules.
        match item.node {
            ast::ItemKind::Mod(_) |
            ast::ItemKind::Struct(_, _) => {
                self.push_path(item.ident);
            },
            _ => (),
        }

        self.items.push(item);
        visit::walk_item(self, item);
        self.items.pop();

        match item.node {
            ast::ItemKind::Mod(_) |
            ast::ItemKind::Struct(_, _) => {
                self.pop_path()
            },
            _ => (),
        }
    }
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
        remove_dir_all(&crate_doc_path)
            .chain_err(|| format!("Could not remove crate doc directory {}", &crate_doc_path.display()))?;
    }

    let mut visitor = RustdocCacher {
        store: Store::new(crate_doc_path).unwrap(),
        current_scope: ModPath(Vec::new()),
        crate_info: crate_info.clone(),
        items: Vec::new(),
    };

    // Push the crate name onto the current namespace so
    // the module "module" will resolve to "crate::module"
    visitor.current_scope.push(PathSegment{
        identifier: crate_info.package.name.clone()
    });

    visitor.visit_mod(&krate.module, krate.span, ast::CRATE_NODE_ID);

    Ok(visitor.store)
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let store = test_harness(r#"
        mod a {
            mod b {
            }
        }"#).unwrap();
        let modules = store.get_modules();
        // let p = &ModPath::from("test".to_string());
        // assert!(modules.contains(p), true);
        // let p = &ModPath::from("test::a".to_string());
        // assert!(modules.contains(p), true);
        // let p = &ModPath::from("test::a::b".to_string());
        // assert!(modules.contains(p), true);
    }

    #[test]
    fn test_module_contains_fns() {
        let store = test_harness(r#"
        mod a {
            mod b {
                fn thing() {
                    println!("Hello, world!");
                }
            }
        }"#).unwrap();
        panic!("not ready")
    }
}
