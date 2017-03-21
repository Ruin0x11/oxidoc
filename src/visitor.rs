use std::path::PathBuf;

use syntax::abi;
use syntax::ast;
use syntax::print::pprust;
use syntax::parse::{self, ParseSess};

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
pub struct OxidocVisitor<'a> {
    pub current_scope: ModPath,
    pub ctxt: &'a Context,
    pub crate_module: Module,
}

impl<'a> OxidocVisitor<'a> {
    pub fn new(ctxt: &'a Context) -> OxidocVisitor<'a> {
        OxidocVisitor {
            crate_module: Module::new(None),
            current_scope: ModPath::new(),
            ctxt: ctxt,
        }
    }

    fn make_modpath(&self, ident: ast::Ident) -> ModPath {
        let mut path = self.current_scope.clone();
        let name = pprust::ident_to_string(ident);
        path.push_string(name);
        path
    }

    fn visit_enum_def(&mut self, item: &ast::Item,
                      enum_def: &ast::EnumDef,
                      generics: &ast::Generics) -> Enum {
        Enum {
            ident: item.ident,
            variants: enum_def.variants.clone(),
            attrs: item.attrs.clone(),
            path: self.make_modpath(item.ident),
        }
    }

    fn visit_fn(&mut self, item: &ast::Item,
                fn_decl: &ast::FnDecl,
                ast_unsafety: ast::Unsafety,
                ast_constness: ast::Constness,
                ast_abi: abi::Abi,
                generics: &ast::Generics) -> Function {
        Function {
            ident: item.ident,
            unsafety: ast_unsafety,
            constness: ast_constness,
            visibility: ast::Visibility::Inherited,
            abi: ast_abi,
            attrs: item.attrs.clone(),
            path: self.make_modpath(item.ident),
        }
    }

    fn visit_const(&self, item: &ast::Item,
                   ast_ty: &ast::Ty,
                   ast_expr: &ast::Expr,
                   ) -> Constant {
        Constant {
            ident: item.ident,
            type_: ast_ty.clone(),
            expr:  ast_expr.clone(),
            attrs: item.attrs.clone(),
            path: self.make_modpath(item.ident),
        }
    }

    fn visit_struct(&self, item: &ast::Item,
                    variant_data: &ast::VariantData,
                    ast_generics: &ast::Generics) -> Struct {
        Struct {
            ident: item.ident,
            fields: variant_data.fields().iter().cloned().collect(),
            attrs: item.attrs.clone(),
            path: self.make_modpath(item.ident),
        }

    }

    fn visit_trait(&self, item: &ast::Item,
                   ast_unsafety: ast::Unsafety,
                   ast_generics: &ast::Generics,
                   trait_items: &Vec<ast::TraitItem>) -> Trait {
        Trait {
            ident: item.ident,
            unsafety: ast_unsafety,
            attrs: item.attrs.clone(),
            path: self.make_modpath(item.ident),
        }
    }

    fn visit_impl(&self, item: &ast::Item,
                  ast_unsafety: ast::Unsafety,
                  ast_generics: &ast::Generics,
                  ast_trait_ref: &Option<ast::TraitRef>,
                  ty: &ast::Ty,
                  items: &Vec<ast::ImplItem>) -> Impl {
        Impl {
            unsafety: ast_unsafety,
            trait_: ast_trait_ref.clone(),
            for_: ty.clone(),
            items: items.clone(),
            attrs: item.attrs.clone(),
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

    fn visit_item(&mut self, item: &ast::Item, module: &mut Module) {
        match item.node {
            ast::ItemKind::Use(ref view_path) => {
                // TODO: Resolve 'use'd paths
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
                module.enums.push(e);
            },
            ast::ItemKind::Struct(ref variant_data, ref generics) => {
                let s = self.visit_struct(item, 
                                          variant_data,
                                          generics);
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
            ast::ItemKind::Ty(ref ty, ref generics) => {

            },
            ast::ItemKind::Static(ref ty, mutability, ref expr) => {

            },
            ast::ItemKind::Mac(..) |
            ast::ItemKind::ExternCrate(..) |
            ast::ItemKind::ForeignMod(..) => (),
        }
    }

    fn visit_module(&mut self, attrs: Vec<ast::Attribute>, m: &ast::Mod,
                    mod_name: Option<ast::Ident>) -> Module {
        let mut module = Module::new(mod_name);
        module.attrs = attrs.clone();

        for item in &m.items {
            self.visit_item(item, &mut module);
        }

        module
    }

    pub fn visit_crate(&mut self, krate: ast::Crate) {
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

        let context = Context {
            store_path: PathBuf::from("~/.cargo/registry/doc/test-0.1.0"),
        };

        let mut visitor = OxidocVisitor::new(&context);
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
