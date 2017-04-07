use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;

use syntax::abi;
use syntax::ast;
use syntax::print::pprust;
use syntax::parse::{self, ParseSess};
use syntax::symbol::keywords;

use document::*;

use errors::*;
use convert::Context;

/// Visits the AST starting at a crate and creates a tree of documentation
/// items. These will later be flattened into a single Store so that no
/// tree traversals are necessary during lookup.
///
/// Does not implement "Visitor" since this design allows passing in found Items
/// as arguments instead of maintaining a global stack of Items and looking at
/// the last one found.
pub struct OxidocVisitor {
    pub current_scope: ModPath,
    pub crate_info: CrateInfo,
    pub crate_module: Module,
    pub name_for_ty: HashMap<NodeId, ast::Ident>,
    pub impls_for_ty: HashMap<ModPath, Vec<Impl>>,
}

impl OxidocVisitor {
    pub fn new(crate_info: CrateInfo) -> OxidocVisitor {
        OxidocVisitor {
            crate_module: Module::new(None),
            current_scope: ModPath::new(),
            crate_info: crate_info,
            name_for_ty: HashMap::new(),
            impls_for_ty: HashMap::new(),
        }
    }

    fn visit_impl_const(&self, item: &ast::ImplItem, for_path: &ModPath, ty: &ast::Ty, expr: &ast::Expr) -> Constant {
        Constant {
            ident: item.ident,
            type_: Ty::from(ty.clone()),
            expr: expr.clone(),
            vis: item.vis.clone(),
            attrs: item.attrs.clone(),
            path: for_path.append_ident(item.ident),
        }
    }

    fn visit_impl_method(&self, item: &ast::ImplItem, for_path: &ModPath, sig: &ast::MethodSig) -> Function {
        // In this case, the final segment of the ModPath is used as the type
        // the item is implemented on.
        Function {
            ident: item.ident,
            decl: (*sig.decl).clone(),
            unsafety: sig.unsafety.clone(),
            constness: sig.constness.node.clone(),
            vis: item.vis.clone(),
            abi: sig.abi.clone(),
            attrs: item.attrs.clone(),
            kind: FnKind::MethodFromImpl,
            path: for_path.append_ident(item.ident),
        }
    }

    fn visit_impl_item(&self, module: &mut Module, item: &ast::ImplItem, for_path: &ModPath) {
        match item.node {
            ast::ImplItemKind::Const(ref ty, ref expr) => {
                let c = self.visit_impl_const(item, for_path, ty, expr);
                module.consts.push(c);
            },
            ast::ImplItemKind::Method(ref sig, ref block) => {
                let f = self.visit_impl_method(item, for_path, sig);
                module.fns.push(f);
            },
            // TODO: Handle types and macros
            ast::ImplItemKind::Type(ref ty) => (),
            ast::ImplItemKind::Macro(ref mac) => (),
        }
    }

    fn add_impl(&mut self, module: &mut Module, imp: Impl) {
        if let ast::TyKind::Path(_, path) = imp.for_.node.clone() {
            let namespaced_path = ModPath::from(path.clone());
            if let Some(full_path) = module.resolve_use(&namespaced_path) {
                println!("Full path: {}", full_path);
                for item in &imp.items {
                    self.visit_impl_item(module, &item, &full_path);
                }
                self.impls_for_ty.entry(full_path.clone()).or_insert(Vec::new()).push(imp);
            } else {
                println!("No type found for impl {}", namespaced_path);
            }
        }
    }

    fn visit_enum_def(&self, item: &ast::Item,
                      enum_def: &ast::EnumDef,
                      _generics: &ast::Generics) -> Enum {
        Enum {
            ident: item.ident,
            vis: item.vis.clone(),
            variants: enum_def.variants.clone(),
            attrs: item.attrs.clone(),
            path: self.current_scope.append_ident(item.ident),
        }
    }

    fn visit_fn(&self, item: &ast::Item,
                fn_decl: &ast::FnDecl,
                ast_unsafety: ast::Unsafety,
                ast_constness: ast::Constness,
                ast_abi: abi::Abi,
                _generics: &ast::Generics) -> Function {
        Function {
            ident: item.ident,
            decl: fn_decl.clone(),
            unsafety: ast_unsafety,
            constness: ast_constness,
            vis: item.vis.clone(),
            abi: ast_abi,
            attrs: item.attrs.clone(),
            kind: FnKind::ItemFn,
            path: self.current_scope.append_ident(item.ident),
        }
    }

    fn visit_const(&self, item: &ast::Item,
                   ast_ty: &ast::Ty,
                   ast_expr: &ast::Expr,
    ) -> Constant {
        Constant {
            ident: item.ident,
            type_: Ty::from(ast_ty.clone()),
            expr:  ast_expr.clone(),
            vis: item.vis.clone(),
            attrs: item.attrs.clone(),
            path: self.current_scope.append_ident(item.ident),
        }
    }

    fn visit_struct(&self, item: &ast::Item,
                    variant_data: &ast::VariantData,
                    _ast_generics: &ast::Generics) -> Struct {
        Struct {
            ident: item.ident,
            id: NodeId::from(item.id),
            vis: item.vis.clone(),
            fields: variant_data.fields().iter().cloned().collect(),
            attrs: item.attrs.clone(),
            path: self.current_scope.append_ident(item.ident),
        }

    }

    fn visit_trait(&self, item: &ast::Item,
                   ast_unsafety: ast::Unsafety,
                   _ast_generics: &ast::Generics,
                   trait_items: &Vec<ast::TraitItem>) -> Trait {
        Trait {
            items: trait_items.iter().cloned().map(|ti| {
                TraitItem {
                    ident: ti.ident,
                    attrs: ti.attrs.clone(),
                    path: ModPath::join(&self.current_scope.append_ident(item.ident),
                                        &ModPath::from(ti.ident)),
                    node: ti.node,
                }
            }).collect(),
            ident: item.ident,
            unsafety: ast_unsafety,
            vis: item.vis.clone(),
            attrs: item.attrs.clone(),
            path: self.current_scope.append_ident(item.ident),
        }
    }

    fn visit_impl(&self, item: &ast::Item,
                  ast_unsafety: ast::Unsafety,
                  _ast_generics: &ast::Generics,
                  ast_trait_ref: &Option<ast::TraitRef>,
                  ast_ty: &ast::Ty,
                  items: &Vec<ast::ImplItem>) -> Impl {
        Impl {
            unsafety: ast_unsafety,
            trait_: ast_trait_ref.clone(),
            for_: ast_ty.clone(),
            items: items.clone(),
            attrs: item.attrs.clone(),
            path: self.current_scope.append_ident(item.ident)
        }
    }

    fn visit_default_impl(&self, item: &ast::Item,
                          ast_unsafety: ast::Unsafety,
                          ast_trait_ref: &ast::TraitRef) -> DefaultImpl {
        DefaultImpl {
            unsafety: ast_unsafety,
            trait_: ast_trait_ref.clone(),
            attrs: item.attrs.clone(),
        }
    }

    fn add_uses(&self, module: &mut Module,
                _item: &ast::Item,
                import: &ast::ViewPath) {
        // TODO: This will take some work to resolve globbed imports from
        // external crates.
        match import.node {
            ast::ViewPath_::ViewPathSimple(ident, ref path) => {
                module.add_use(&ident, ModPath::from(path.clone()));
            },
            ast::ViewPath_::ViewPathGlob(ref path) => {
                // FIXME: Get all the keywords for this namespace. One would
                // have to look into stores of dependencies that are already
                // saved and get the list of namespaces there.
                //
                // I couldn't make this function pure, and therefore testable,
                // because this case would have to return something.
            },
            ast::ViewPath_::ViewPathList(ref path, ref items) => {
                for item in items {
                    // Unlike ViewPathSimple, the path does not contain each
                    // ident at the end, so it must be added.
                    let ident = match item.node.rename {
                        Some(ren) => ren,
                        None      => {
                            if item.node.name == keywords::SelfValue.ident() {
                                // 'self' means the final namespace part of the
                                // path resolves to the global path.
                                path.segments.last().unwrap().identifier
                            } else {
                                item.node.name
                            }
                        },
                    };

                    let full_path = if path.segments.last().unwrap().identifier == ident {
                        // This was originally the 'self' keyword, so
                        // std::fmt::{self} becomes
                        // path = std::fmt, ident = fmt
                        ModPath::from(path.clone())
                    } else {
                        ModPath::join(&ModPath::from(path.clone()),
                                      &ModPath::from(ident))
                    };
                    module.add_use(&ident, full_path);
                }
            }
        }
    }

    fn visit_item(&mut self, item: &ast::Item, module: &mut Module) {
        match item.node {
            ast::ItemKind::Use(ref view_path) => {
                self.add_uses(module, item, view_path);
            },
            ast::ItemKind::Const(ref ty, ref expr) => {
                let c = self.visit_const(item, ty, expr);
                module.consts.push(c);
            }
            ast::ItemKind::Fn(ref decl, unsafety, constness,
                              abi, ref generics, _) => {
                let f = self.visit_fn(item, &*decl,
                                      unsafety, constness.node,
                                      abi, generics);
                module.fns.push(f);
            },
            ast::ItemKind::Mod(ref mod_) => {
                let m = self.visit_module(item.attrs.clone(),
                                          mod_, Some(item.ident));
                module.mods.push(m);
            },
            ast::ItemKind::Enum(ref def, ref generics) => {
                let e = self.visit_enum_def(item,
                                            def, generics);
                module.add_use(&item.ident, e.path.clone());
                module.enums.push(e);
            },
            ast::ItemKind::Struct(ref variant_data, ref generics) => {
                let s = self.visit_struct(item,
                                          variant_data,
                                          generics);
                module.add_use(&item.ident, s.path.clone());
                module.structs.push(s);
            },
            ast::ItemKind::Union(ref variant_data, ref generics) => {
                // TODO when unions become stable?
            },
            ast::ItemKind::Trait(unsafety, ref generics,
                                 ref param_bounds, ref trait_items) => {
                let t = self.visit_trait(item,
                                         unsafety, generics,
                                         trait_items);
                module.traits.push(t);
            },
            ast::ItemKind::DefaultImpl(unsafety, ref trait_ref) => {
                let def_trait = self.visit_default_impl(item, unsafety,
                                                        trait_ref);
                module.def_traits.push(def_trait);
            },
            ast::ItemKind::Impl(unsafety, polarity,
                                ref generics, ref trait_ref,
                                ref ty, ref items) => {
                let i = self.visit_impl(item, unsafety,
                                        generics, trait_ref,
                                        ty, items);
                module.impls.push(i);
            },
            ast::ItemKind::Ty(..) |
            ast::ItemKind::Static(..) |
            ast::ItemKind::Mac(..) |
            ast::ItemKind::ExternCrate(..) |
            ast::ItemKind::ForeignMod(..) => (),
        }
    }

    fn visit_module(&mut self, attrs: Vec<ast::Attribute>, m: &ast::Mod,
                    mod_name: Option<ast::Ident>) -> Module {
        debug!("visiting module");
        let mut module = Module::new(mod_name);
        module.attrs = attrs.clone();

        if let Some(name) = mod_name {
            self.current_scope.push_string(pprust::ident_to_string(name));
        } else {
            self.current_scope.push_string(pprust::ident_to_string(keywords::CrateRoot.ident()));
        }

        module.path = self.current_scope.clone();

        // FIXME: add module path to be resolved to

        debug!("path: {}", self.current_scope);

        for item in &m.items {
            self.visit_item(item, &mut module);
        }

        // TODO: At this point, the items that could have impls will be added to
        // module.namespaces_to_paths. All the impls will be in a vector. Map
        // the impls.

        self.current_scope.pop();

        while let Some(impl_) = module.impls.pop() {
            self.add_impl(&mut module, impl_);
        }

        module
    }

    pub fn visit_crate(&mut self, krate: ast::Crate) {
        debug!("visiting crate");
        self.crate_module = self.visit_module(krate.attrs.clone(),
                                              &krate.module,
                                              None);
        self.crate_module.is_crate = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use env_logger;
    use std::collections::HashMap;

    fn parse_crate_from_source(source_code: &str,
                               parse_session: ParseSess) -> Result<ast::Crate> {
        match parse::parse_crate_from_source_str("test.rs".to_string(),
                                                 source_code.to_string(),
                                                 &parse_session) {
            Ok(_) if parse_session.span_diagnostic.has_errors()
                => bail!("Parse error"),
            Ok(krate) => Ok(krate),
            Err(_) => bail!("Failed to parse"),
        }
    }

    fn test_harness(source_code: &str) -> Result<Module> {
        let parse_session = ParseSess::new();
        let krate = parse_crate_from_source(source_code, parse_session)?;

        let crate_info = CrateInfo {
            package: Package {
                name: "test".to_string(),
                version: "0.1.0".to_string(),
            }
        };

        let context = Context::new(
            PathBuf::from("~/.cargo/registry/doc/test-0.1.0"),
            crate_info.clone(),
            HashMap::new());

        let mut visitor = OxidocVisitor::new(crate_info);
        visitor.visit_crate(krate);
        Ok(visitor.crate_module)
    }

    #[test]
    fn test_nested_modules() {
        let _ = env_logger::init();
        let module = test_harness(r#"
        mod a {
            mod b {
            }
        }"#).unwrap();
        let first_mod = module.mods.iter().next().unwrap();
        let second_mod = first_mod.mods.iter().next().unwrap();
    }
}
