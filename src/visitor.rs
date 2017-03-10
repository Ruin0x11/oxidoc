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

pub struct RustdocVisitor<'v> {
    pub store: Store,
    pub current_scope: ModPath,
    pub crate_info: CrateInfo,
    pub items: Vec<&'v ast::Item>,

    // If a docstring was found inside an adjacent Item's node,
    // push it here and consume it when the corresponding Item is reached.
    // The docstring and item information are separate from one another.
    pub docstrings: Vec<String>,

    // If something is 'use'd, make sure we can access what it is referring to.
    // Push a new hashmap of identifiers and the global paths they reference
    // upon entering a module and add everything 'use'd to it, and pop it off
    // when it goes out of scope.
    pub is_part_of_use: bool,
    pub used_namespaces: Vec<HashMap<String, ModPath>>
}

/// Possibly retrives a docstring for the specified attributes.
pub fn get_doc(attrs: &Vec<ast::Attribute>) -> Option<String> {
    let mut doc = String::new();
    let mut attrs = attrs.iter().filter(|at| at.check_name("doc")).peekable();
    if let None = attrs.peek() {
        return None;
    }

    let attr = attrs.next().unwrap();
    if let Some(d) = attr.value_str() {
        doc.push_str(&d.to_string());
    }

    while let Some(attr) = attrs.next() {
        if let Some(d) = attr.value_str() {
            doc.push_str(&"\n");
            doc.push_str(&d.to_string());
        }
    }

    info!("Getdoc: {}", doc);
    Some(doc)
}

impl<'v> RustdocVisitor<'v> {
    /// Pushes a path onto the current path scope.
    pub fn push_path(&mut self, scope: ModPath) {
        for seg in scope.0.iter() {
            self.current_scope.push(seg.clone());
        }
    }

    /// Pushes a segment onto the current path scope.
    pub fn push_segment(&mut self, name: String) {
        let seg = PathSegment{ identifier: name };
        self.current_scope.push(seg);
    }

    /// Pops a segment from the current path scope.
    pub fn pop_segment(&mut self) {
        self.current_scope.pop();
    }

    /// Put all of the namespaces in the given 'use' path into the hashmap of known namespaces for the current module.
    pub fn add_use_namespaces(&mut self, vp: &ast::ViewPath) {
        if let Some(namespaces) = self.used_namespaces.last_mut() {
            match vp.node {
                ast::ViewPathSimple(rename, ref path) => {
                    namespaces.insert(pprust::ident_to_string(rename), ModPath::from(path.clone()));
                }
                ast::ViewPathGlob(ref path) => {
                    // TODO: add everything under the globbed path.
                    // the glob could match either a module or enum.
                }
                ast::ViewPathList(ref prefix, ref list) => {
                    // visitor.visit_path(prefix, item.id);
                    // for item in list {
                    //     visitor.visit_path_list_item(prefix, item)
                    // }
                    for item in list {
                        let mut name = pprust::ident_to_string(item.node.name);
                        if name == "{{root}}" {
                            name = self.crate_info.package.name.clone();
                        }
                        namespaces.insert(name, ModPath::from(prefix.clone()));
                    }
                }
            }
        }
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

                let doc = match self.docstrings.pop() {
                    Some(d) => d.to_string(),
                    None    => "".to_string(),
                };

                Some(Document{

                    crate_info: self.crate_info.clone(),
                    path: my_path,
                    signature: sig,
                    docstring: doc,
                    doc: FnDoc_ {
                        unsafety: Unsafety::from(unsafety),
                        constness: Constness::from(constness),
                        // TODO: Generics
                        visibility: Visibility::from(visibility.clone()),
                        abi: Abi::from(abi),
                        ty: FnKind::ItemFn,
                    }})
            },
            visit::FnKind::Method(id, m, vis, block) => {
                let mut part_of_impl = false;

                let mut name: String = String::new();
                let my_ty = if let Some(item) = self.items.iter().last() {
                    match item.node {
                        ast::ItemKind::Mod(_) |
                        ast::ItemKind::Struct(_, _) => {
                            FnKind::Method
                        },
                        ast::ItemKind::DefaultImpl(_, _) => {
                            FnKind::MethodFromTrait
                        },
                        ast::ItemKind::Impl(_, _, _, _, ref ty, _) => {
                            name = pprust::ty_to_string(ty);
                            part_of_impl = true;
                            FnKind::MethodFromImpl
                        }
                        _ => {
                            FnKind::ItemFn
                        },
                    }
                } else {
                    FnKind::ItemFn
                };

                // Save the name of the struct inside the documentation path
                // if the function is inside that struct's impl
                // The name itself can have a module path like "module::Struct"
                // if "impl module::Struct" is given
                if part_of_impl {
                    self.push_path(ModPath::from(name.clone()));
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

                let sig = pprust::to_string(|s| s.print_method_sig(id, &m, &visibility));

                let doc = match self.docstrings.pop() {
                    Some(d) => d.to_string(),
                    None    => "".to_string(),
                };

                let fn_doc = Some(FnDoc {
                    crate_info: self.crate_info.clone(),
                    path: my_path,
                    signature: sig,
                    docstring: doc,
                    doc: FnDoc_ {
                        unsafety: Unsafety::from(m.unsafety),
                        constness: Constness::from(m.constness.node),
                        // TODO: Generics
                        visibility: Visibility::from(visibility),
                        abi: Abi::from(m.abi),
                        ty: my_ty,
                    }
                });

                if part_of_impl {
                    for _ in 0..ModPath::from(name.clone()).0.len() {
                        self.pop_segment();
                    }
                }

                fn_doc

            },
            visit::FnKind::Closure(_) => None // Don't care.
        }
    }

}

impl<'v> Visitor<'v> for RustdocVisitor<'v> {
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


    fn visit_impl_item(&mut self, ii: &'v ast::ImplItem)  {
        // Docstrings may also live inside ImplItems as well as Items.
        if let Some(doc) = get_doc(&ii.attrs) {
            info!("Getdoc: {}", doc);
            self.docstrings.push(doc);
        } else {
            self.docstrings.push("".to_string());
        }
        visit::walk_impl_item(self, ii);
    }

    fn visit_variant_data(&mut self, var: &'v ast::VariantData, id: ast::Ident,
                          _: &'v ast::Generics, node_id: ast::NodeId, span: Span) {

        let sig = match self.items.iter().last() {
            Some(item) => format!("{} {} {{ /* fields omitted */ }}",
                                  pprust::visibility_qualified(&item.vis, &"struct"),
                                  pprust::ident_to_string(id)),
            None       => format!("struct {} {{ /* fields omitted */ }}",
                                  pprust::ident_to_string(id)),

        };

        let doc = match self.docstrings.pop() {
            Some(d) => d.to_string(),
            None    => "".to_string(),
        };

        let struct_doc = Document {
            crate_info: self.crate_info.clone(),
            // The current scope itself contains the struct name as the last segment,
            // which is the directory where we want the struct documentation to live.
            path: self.current_scope.clone(),
            signature: sig,
            docstring: doc,
            doc: StructDoc_ {
                fn_docs: Vec::new(),
            }
        };

        self.store.add_struct(struct_doc);

        visit::walk_struct_def(self, var);
    }

    fn visit_mod(&mut self, m: &'v ast::Mod, _s: Span, _n: ast::NodeId) {
        let sig = format!("mod {}", self.current_scope);

        let doc = match self.docstrings.pop() {
            Some(d) => d.to_string(),
            None    => "".to_string(),
        };

        let mod_doc = Document {
            crate_info: self.crate_info.clone(),
            path: self.current_scope.clone(),
            signature: sig,
            docstring: doc,
            doc: ModuleDoc_ {
                fn_docs: Vec::new(),
                struct_docs: Vec::new(),
                module_docs: Vec::new(),
            }
        };

        self.store.add_module(mod_doc);

        visit::walk_mod(self, m);
    }

    fn visit_item(&mut self, item: &'v ast::Item) {
        // Keep track of the path we're in as we traverse modules.
        match item.node {
            ast::ItemKind::Mod(_) => {
                info!("ITEMKIND: mod");

                self.push_segment(pprust::ident_to_string(item.ident));
                self.store.add_modpath(self.current_scope.clone());

                // Keep track of what is 'use'd inside this module
                self.used_namespaces.push(HashMap::new());
            },
            ast::ItemKind::Enum(_, _) |
            ast::ItemKind::Struct(_, _) => {
                // Let the struct/enum name be a path that can be resolved to
                info!("ITEMKIND: struct/enum");
                self.push_segment(pprust::ident_to_string(item.ident));
            },
            ast::ItemKind::Use(ref vp) => {
                info!("ITEMKIND: use");
                self.add_use_namespaces(vp);
            },
            ast::ItemKind::Impl(_, _, _, _, _, _) |
            ast::ItemKind::DefaultImpl(_, _) => {
                info!("ITEMKIND: impl");
                // TODO: Need to record the trait the impl is from and the type it is on
            },
            ast::ItemKind::Trait(unsafety, generics, bounds, items) => {
                info!("ITEMKIND: trait");
                let name = pprust::ident_to_string(item.ident);
                self.push_segment(name);
                let sig = format!("trait {}", name);
                let t = Document {
                    crate_info: self.crate_info.clone(),
                    // The current scope itself contains the trait name as the last segment,
                    // which is the directory where we want the struct documentation to live.
                    path: self.current_scope.clone(),
                    signature: sig,
                    docstring: self.docstrings.pop().unwrap(),
                    doc: TraitDoc_ {
                        unsafety: Unsafety::from(unsafety),
                    }
                };
                self.store.add_trait(t);
            },
            _ => info!("ITEMKIND: something else"),
        }

        self.items.push(item);

        if let Some(doc) = get_doc(&item.attrs) {
            self.docstrings.push(doc);
        } else {
            self.docstrings.push("".to_string());
        }

        visit::walk_item(self, item);

        self.items.pop();

        match item.node {
            ast::ItemKind::Mod(_) => {
                self.pop_segment();

                // 'use'd namespaces go out of scope
                self.used_namespaces.pop();
            }
            ast::ItemKind::Trait(_, _, _, _) |
            ast::ItemKind::Enum(_, _) |
            ast::ItemKind::Struct(_, _) => {
                self.pop_segment()
            }
            ast::ItemKind::Use(_) => {
                self.is_part_of_use = false;
            },
            ast::ItemKind::Impl(_, _, _, _, _, _) |
            ast::ItemKind::DefaultImpl(_, _) => {
            }
            _ => (),
        }
    }
}
