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

    fn make_modpath(&self, name: String) -> ModPath {
        let path = self.current_scope.clone();
        path.push_string(name);
        path
    }

    fn visit_enum_def(&mut self, item: &ast::Item,
                      name: String,
                      enum_def: &ast::EnumDef,
                      generics: &ast::Generics) -> Enum {
        Enum {
            name: name,
            variants: enum_def.variants.iter().cloned().map(|x| Variant::from(x)).collect(),
            attrs: Attributes::from_ast(&item.attrs),
            path: self.make_modpath(name),
        }
    }

    fn visit_fn(&mut self, item: &ast::Item,
                name: String,
                fn_decl: &ast::FnDecl,
                ast_unsafety: ast::Unsafety,
                ast_constness: ast::Constness,
                ast_abi: abi::Abi,
                generics: &ast::Generics) -> Function {
        Function {
            name: name,
            unsafety: Unsafety::from(ast_unsafety),
            constness: Constness::from(ast_constness),
            visibility: Visibility::Inherited,
            abi: Abi::from(ast_abi),
            attrs: Attributes::from_ast(&item.attrs),
            path: self.make_modpath(name),
        }
    }

    fn visit_const(&self, item: &ast::Item,
                   ast_ty: &ast::Ty,
                   ast_expr: &ast::Expr,
                   name: String) -> Constant {
        Constant {
            type_: pprust::to_string(|s| s.print_type(ast_ty)),
            expr:  pprust::to_string(|s| s.print_expr_maybe_paren(ast_expr)),
            name:  name,
            attrs: Attributes::from_ast(&item.attrs),
            path: self.make_modpath(name),
        }
    }

    fn visit_struct(&self, item: &ast::Item,
                    name: String,
                    variant_data: &ast::VariantData,
                    ast_generics: &ast::Generics) -> Struct {
        Struct {
            name: name,
            fields: StructField::from_variant_data(variant_data.fields()),
            attrs: Attributes::from_ast(&item.attrs),
            path: self.make_modpath(name),
        }
        
    }

    fn visit_trait(&self, item: &ast::Item,
                   name: String,
                   ast_unsafety: ast::Unsafety,
                   ast_generics: &ast::Generics,
                   trait_items: &Vec<ast::TraitItem>) -> Trait {
        Trait {
            name: name,
            unsafety: Unsafety::from(ast_unsafety),
            attrs: Attributes::from_ast(&item.attrs),
            path: self.make_modpath(name),
        }
    }

    fn visit_impl(&self, item: &ast::Item,
                  ast_unsafety: ast::Unsafety,
                  ast_generics: &ast::Generics,
                  ast_trait_ref: &Option<ast::TraitRef>,
                  ty: &ast::Ty,
                  items: &Vec<ast::ImplItem>) -> Impl {
        let trait_ = match *ast_trait_ref {
            Some(ref tr) => Some(TraitRef::from(tr.clone())),
            None     => None,
        };
        Impl {
            unsafety: Unsafety::from(ast_unsafety),
            trait_: trait_,
            for_: Ty::from(ty.clone()),
            items: items.iter().map(|x| ImplItem {
                // TODO: implement
            }).collect(),
            attrs: Attributes::from_ast(&item.attrs),
        }
    }

    fn visit_default_impl(&self, item: &ast::Item,
                          ast_unsafety: ast::Unsafety,
                          ast_trait_ref: &ast::TraitRef) -> DefaultImpl {
        DefaultImpl {
            unsafety: Unsafety::from(ast_unsafety),
            trait_: TraitRef::from(ast_trait_ref.clone()),
            attrs: Attributes::from_ast(&item.attrs),
        }
    }

    fn visit_item(&mut self, item: &ast::Item, module: &mut Module) {
        let name = pprust::ident_to_string(item.ident);
        match item.node {
            ast::ItemKind::Use(ref view_path) => {
                // TODO: Resolve 'use'd paths
            },
            ast::ItemKind::Const(ref ty, ref expr) => {
                let c = self.visit_const(item, ty, expr, name);
                module.consts.push(c);
            }
            ast::ItemKind::Fn(ref decl, unsafety, constness,
                              abi, ref generics, _) => {
                let f = self.visit_fn(item, name, &*decl,
                                      unsafety, constness.node,
                                      abi, generics);
                module.fns.push(f);
            },
            ast::ItemKind::Mod(ref m) => {
                let m = self.visit_module(item.attrs.clone(),
                                          m, Some(name));
                module.mods.push(m);
            },
            ast::ItemKind::Enum(ref def, ref generics) => {
                let e = self.visit_enum_def(item, name,
                                            def, generics);
                module.enums.push(e);
            },
            ast::ItemKind::Struct(ref variant_data, ref generics) => {
                let s = self.visit_struct(item, name,
                                          variant_data,
                                          generics);
                module.structs.push(s);
            },
            ast::ItemKind::Union(ref variant_data, ref generics) => {
                // TODO when unions become stable?
            },
            ast::ItemKind::Trait(unsafety, ref generics,
                                 ref param_bounds, ref trait_items) => {
                let t = self.visit_trait(item, name,
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
                    mod_name: Option<String>) -> Module {
        let mut module = Module::new(mod_name);
        module.attrs = Attributes::from_ast(&attrs);

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
        assert_eq!(
            first_mod.name,
            Some("a".to_string())
        );
        let second_mod = first_mod.mods.iter().next().unwrap();
        assert_eq!(
            second_mod.name,
            Some("b".to_string())
        );
    }
}
