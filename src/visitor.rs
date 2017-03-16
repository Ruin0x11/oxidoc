use syntax::abi;
use syntax::ast;
use syntax::print::pprust;
use syntax::parse::{self, ParseSess};

use document::*;

use errors::*;

/// Visits the AST starting at a crate and creates a tree of documentation
/// items. These will later be flattened into a single Store so that no
/// tree traversals are necessary during lookup.
///
/// Does not implement "Visitor" since this design allows passing in found Items
/// as arguments instead of maintaining a global stack of Items and looking at
/// the last one found.
pub struct OxidocVisitor {
    pub current_scope: ModPath,
}

impl OxidocVisitor {
    fn new() -> OxidocVisitor {
        OxidocVisitor {
            current_scope: ModPath::new(),
        }
    }

    fn visit_enum_def(&mut self, item: &ast::Item,
                      name: Option<String>, def: &ast::EnumDef,
                      generics: &ast::Generics) -> EnumDoc {
        EnumDoc { }
    }

    fn visit_fn(&mut self, item: &ast::Item,
                name: Option<String>,
                fn_decl: &ast::FnDecl,
                ast_unsafety: ast::Unsafety,
                ast_constness: ast::Constness,
                ast_abi: abi::Abi,
                generics: &ast::Generics) -> FnDoc {
        FnDoc {
            unsafety: Unsafety::from(ast_unsafety),
            constness: Constness::from(ast_constness),
            visibility: Visibility::Inherited,
            abi: Abi::from(ast_abi),
            ty: FnKind::ItemFn, //TODO: fix
        }
    }

    fn visit_item(&mut self, item: &ast::Item, module_doc: &mut ModuleDoc) {
        let name = pprust::ident_to_string(item.ident);
        match item.node {
            ast::ItemKind::Use(ref view_path) => {
                
            },
            ast::ItemKind::Const(ref ty, ref expr) => {
                
            }
            ast::ItemKind::Fn(ref decl, unsafety, constness,
                              abi, ref generics, _) => {
                module_doc.fns.push(self.visit_fn(item, Some(name), &*decl,
                                                  unsafety, constness.node,
                                                  abi, generics))
            },
            ast::ItemKind::Mod(ref m) => {
                module_doc.mods.push(self.visit_module(item.attrs.clone(),
                                                              m,
                                                              Some(name)));
            },
            ast::ItemKind::Enum(ref def, ref generics) => {
                self.visit_enum_def(item, Some(name), def, generics);
            },
            ast::ItemKind::Struct(ref variant_data, ref generics) => {
                
            },
            ast::ItemKind::Union(ref variant_data, ref generics) => {
                
            },
            ast::ItemKind::Trait(unsafety, ref generics,
                                 ref param_bounds, ref items) => {
                
            },
            ast::ItemKind::DefaultImpl(unsafety, ref trait_ref) => {
                
            },
            ast::ItemKind::Impl(unsafety, polarity, ref generics, ref trait_ref,
                                ref ty, ref items) => {
                
            },
            ast::ItemKind::Ty(ref ty, ref generics) => {
                
            },
            ast::ItemKind::Static(ref ty, mutability, ref expr) => {
                
            },
            _ => (),
        }
    }

    fn visit_module(&mut self, attrs: Vec<ast::Attribute>, module: &ast::Mod,
                    mod_name: Option<String>) -> ModuleDoc {
        let mut module_doc = ModuleDoc::new(mod_name);

        for item in &module.items {
            self.visit_item(item, &mut module_doc);
        }

        module_doc
    }

    fn visit_crate(&mut self, krate: ast::Crate) -> ModuleDoc {
        let mut crate_module = self.visit_module(krate.attrs.clone(),
                                             &krate.module,
                                             None);
        crate_module.is_crate = true;
        crate_module
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use env_logger;

    fn parse_crate_from_source(source_code: &str, parse_session: ParseSess) -> Result<ast::Crate> {
        match parse::parse_crate_from_source_str("test.rs".to_string(),
                                                 source_code.to_string(),
                                                 &parse_session) {
            Ok(_) if parse_session.span_diagnostic.has_errors() => bail!("Parse error"),
            Ok(krate) => Ok(krate),
            Err(_) => bail!("Failed to parse"),
        }
    }
    
    fn test_harness(source_code: &str) -> Result<ModuleDoc> {
        let parse_session = ParseSess::new();
        let krate = parse_crate_from_source(source_code, parse_session)?;

        let mut visitor = OxidocVisitor::new();
        let module_doc = visitor.visit_crate(krate);
        Ok(module_doc)
    }

    #[test]
    fn test_nested_modules() {
        let _ = env_logger::init();
        let module_doc = test_harness(r#"
        mod a {
            mod b {
            }
        }"#).unwrap();
        let first_mod = module_doc.mods.iter().next().unwrap();
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
