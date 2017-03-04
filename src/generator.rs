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
use syntax::visit::{self, Visitor};

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
    pub fn push_path(&mut self, name: String) {
        let seg = PathSegment{ identifier: name };
        self.current_scope.push(seg);
    }

    /// Pops a segment from the current path scope.
    pub fn pop_path(&mut self) {
        self.current_scope.pop();
    }

    /// Possibly generates function documentation for the given AST info, or not if it's a closure.
    pub fn convert_fn(&mut self, span: Span,
               fn_kind: visit::FnKind<'v>, fn_decl: &'v ast::FnDecl) -> Option<FnDoc> {

        match fn_kind {
            visit::FnKind::ItemFn(id, gen, unsafety,
                                  Spanned{ node: constness, span: _ }, abi, visibility, _) => {
                let sig = pprust::to_string(|s| s.print_fn(fn_decl, unsafety, constness,
                                                           abi, Some(id), gen, visibility));

                let my_path = ModPath::join(&self.current_scope, &ModPath::from_ident(span, id));

                Some(FnDoc {
                    crate_info: self.crate_info.clone(),
                    path: my_path,
                    signature: sig,
                    unsafety: Unsafety::from(unsafety),
                    constness: Constness::from(constness),
                    // TODO: Generics
                    visibility: Visibility::from(visibility.clone()),
                    abi: Abi::from(abi),
                    ty: FnKind::ItemFn,
                })
            },
            visit::FnKind::Method(id, m, vis, block) => {
                //TODO: This makes sense only in the context of an impl / Trait
                println!("METHOD: {}", pprust::ident_to_string(id));

                let mut part_of_impl = false;

                let mut name: String = String::new();
                let my_ty = if let Some(item) = self.items.iter().last() {
                    match item.node {
                        ast::ItemKind::Mod(_) |
                        ast::ItemKind::Struct(_, _) => {
                            println!("Method inside scope");
                            FnKind::Method   
                        },
                        ast::ItemKind::DefaultImpl(_, _) => {
                            println!("Method on default impl");
                            FnKind::MethodFromTrait   
                        },
                        ast::ItemKind::Impl(_, _, _, _, ref ty, _) => {
                            name = pprust::ty_to_string(ty);
                            part_of_impl = true;
                            println!("Method on impl {}", &name);
                            FnKind::MethodFromImpl
                        }
                        _ => {
                            println!("Something else");
                            FnKind::ItemFn
                        },
                    }
                } else {
                    FnKind::ItemFn
                };

                // Save the name of the struct inside the documentation path
                // if the function is inside that struct's impl
                if part_of_impl {
                    self.push_path(name.clone());
                }

                let visibility = match vis {
                    Some(v) => {
                        v.clone()
                    }
                    None => {
                        ast::Visibility::Inherited
                    }
                };

                let my_path = ModPath::join(&self.current_scope, &ModPath::from_ident(span, id));
                println!("PATH: {}", my_path);

                let sig = pprust::to_string(|s| s.print_method_sig(id, &m, &visibility));

                let fn_doc = Some(FnDoc {
                    crate_info: self.crate_info.clone(),
                    path: my_path,
                    signature: sig,
                    unsafety: Unsafety::from(m.unsafety),
                    constness: Constness::from(m.constness.node),
                    // TODO: Generics
                    visibility: Visibility::from(visibility),
                    abi: Abi::from(m.abi),
                    ty: my_ty,
                });

                if part_of_impl {
                    self.pop_path();
                }

                fn_doc

            },
            visit::FnKind::Closure(_) => None // Don't care.
        }
    }

}

impl<'v> Visitor<'v> for RustdocCacher<'v> {
    //fn visit_fn(&mut self,
    //fk: FnKind<'ast>, fd: &'ast FnDecl, s: Span, _: NodeId) {
    fn visit_fn(&mut self,
                fn_kind: visit::FnKind<'v>,
                fn_decl: &'v ast::FnDecl,
                //block: &'v ast::Block,
                span: Span,
                _id: ast::NodeId) {
        if let Some(fn_doc) = self.convert_fn(span, fn_kind, fn_decl) {
            self.store.add_function(fn_doc);
        }

        // Continue walking the rest of the funciton so we pick up any functions
        // or closures defined in its body.
        visit::walk_fn(self, fn_kind, fn_decl, span);
    }

    fn visit_mac(&mut self, _mac: &'v ast::Mac) {
        // TODO: Record macros
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
                self.push_path(pprust::ident_to_string(item.ident));
                self.items.push(item);
            },
            ast::ItemKind::Impl(_, _, _, _, _, _) |
            ast::ItemKind::DefaultImpl(_, _) => {
                // TODO: Need to record the trait the impl is from and the type it is on
                self.items.push(item);
            }
            _ => (),
        }

        visit::walk_item(self, item);

        match item.node {
            ast::ItemKind::Mod(_) |
            ast::ItemKind::Struct(_, _) => {
                self.items.pop();
                self.pop_path()
            }
            ast::ItemKind::Impl(_, _, _, _, _, _) |
            ast::ItemKind::DefaultImpl(_, _) => {
                self.items.pop();
            }
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
        let p = &ModPath::from("test".to_string());
        assert!(modules.contains(p), true);
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
