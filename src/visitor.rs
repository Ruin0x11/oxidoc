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

error_chain! {
    
}

/// Visits the AST and creates a tree of documentation items.
/// These will later be flattened into a single Store so that no tree traversals
/// are necessary.
///
/// Does not implement "Visitor" since this design allows passing in found Items
/// as arguments instead of maintaining a global stack of Items and looking at
/// the last one found.
pub struct OxidocVisitor {
    pub store: Store,
    pub current_scope: ModPath,
}

impl OxidocVisitor {
    fn new(store_directory: PathBuf) -> OxidocVisitor {
        OxidocVisitor {
            store: Store::new(store_directory).unwrap(),
            current_scope: ModPath::new(),
        }
    }

    fn visit_item(&mut self, item: &ast::Item, module_doc: &mut ModuleDoc) {
        let name = pprust::ident_to_string(item.ident);
        match item.node {
            ast::ItemKind::Mod(ref m) => {
                module_doc.mods.push(self.visit_module(item.attrs.clone(),
                                                              m,
                                                              Some(name)));
            }
        }
    }

    fn visit_module(&mut self, attrs: Vec<ast::Attribute>, module: &ast::Mod,
                    mod_name: Option<String>) -> ModuleDoc {
        let module_doc = ModuleDoc::new(mod_name);

        for item in &module.items {
            self.visit_item(item, &mut module_doc);
        }

        module_doc
    }

    fn visit_crate(&mut self, krate: ast::Crate) -> ModuleDoc {
        let crate_module = self.visit_module(krate.attrs.clone(),
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

        let visitor = OxidocVisitor::new(PathBuf::from("~/.cargo/registry/doc"));
        let module_doc = visitor.visit_crate(krate);
        Ok(module_doc)
    }

    #[test]
    fn test_nested_modules() {
        let _ = env_logger::init();
        let store = test_harness(r#"
        mod a {
            mod b {
            }
        }"#).unwrap();
    }
}
