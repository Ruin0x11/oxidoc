use std::collections::HashMap;
use std::path::PathBuf;
use std::fmt::{self, Display};

use serde::ser::{Serialize};
use serde::de::{Deserialize};
use syntax::abi;
use syntax::ast;
use syntax::print::pprust;
use syntax::ptr::P;

use document::{self, Attributes, CrateInfo, PathSegment, ModPath};
use store::Store;
use visitor::OxidocVisitor;

use convert::wrappers::*;

pub use self::DocInnerData::*;

pub type DocRelatedItems = HashMap<DocType, Vec<DocLink>>;

#[derive(Eq, PartialEq, Debug, Serialize, Deserialize)]
pub struct NewDocTemp_ {
    pub name: String,
    pub attrs: Attributes,
    pub mod_path: ModPath,
    pub inner_data: DocInnerData,
    pub visibility: Option<Visibility>,
    // source code reference
    // References to other documents
    pub links: DocRelatedItems,
}

impl Display for NewDocTemp_ {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.render())
    }
}

impl Display for Visibility {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let vis = match *self {
            Visibility::Public => "pub",
            _                  => "",
        };
        write!(f, "{}", vis)
    }
}

impl DocInnerData {
}

impl NewDocTemp_ {
    fn get_doc_filename(&self) -> String {
        let prefix = self.inner_data.get_doc_file_prefix();
        format!("{}{}.odoc", prefix, self.name)
    }

    fn render(&self) -> String {
        format!(r#"
{}
------------------------------------------------------------------------------
  {}

------------------------------------------------------------------------------

{}

{}
"#,
                self.doc_info(),
                self.inner_data(),
                self.docstring(),
                self.subitems())
    }

    fn doc_info(&self) -> String {
        match self.inner_data {
            DocInnerData::FnDoc(..) |
            DocInnerData::StructDoc(..) |
            DocInnerData::ConstDoc(..) |
            DocInnerData::EnumDoc(..) |
            DocInnerData::TraitDoc(..) => {
                format!("=== In module {}", self.mod_path.parent().unwrap())
            },
            DocInnerData::TraitItemDoc(..) => {
                format!("=== From trait {}", self.mod_path.parent().unwrap())
            }
            DocInnerData::ModuleDoc(ref mod_) => "".to_string(),
        }
    }

    fn docstring(&self) -> String {
        self.attrs.doc_strings.join("\n")
    }

    fn inner_data(&self) -> String {
        let vis_string = match self.visibility {
            Some(ref v) => v.to_string(),
            None    => "".to_string(),
        };

        let header = match self.inner_data {
            DocInnerData::FnDoc(ref func) => {
                format!("fn {} {}", self.name, func.header)
            },
            DocInnerData::ModuleDoc(ref mod_) => {
                format!("mod {}", self.mod_path)
            },
            DocInnerData::EnumDoc(ref enum_) => {
                format!("enum {}", self.name)
            },
            DocInnerData::StructDoc(ref struct_) => {
                format!("struct {} {{ /* fields omitted */ }}", self.name)
            },
            DocInnerData::ConstDoc(ref const_) => {
                format!("const {}: {} = {}", self.name, const_.ty.name, const_.expr)
            },
            DocInnerData::TraitDoc(ref trait_) => {
                format!("trait {} {{ /* fields omitted */ }}", self.name)
            },
            DocInnerData::TraitItemDoc(ref item) => {
                format!("{}", self.trait_item(item))
            },
        };
        format!("{} {}", vis_string, header)
    }

    // TODO: Better way for formatting the wrapped types, as pprust does.
    fn subitems(&self) -> String {
        let categories = match self.inner_data {
            // NOTE: Any better way to just enumerate all DocType values? This
            // violates OCP.
            DocInnerData::ModuleDoc(..) => {
                vec![DocType::Function,
                     DocType::Module,
                     DocType::Enum,
                     DocType::Struct,
                     DocType::Trait,
                     DocType::Const]
            },
            DocInnerData::TraitDoc(..) => {
                vec![DocType::TraitItemConst,
                     DocType::TraitItemMethod,
                     DocType::TraitItemType,
                     DocType::TraitItemMacro]
            },
            DocInnerData::StructDoc(..) => {
                vec![DocType::StructField]
            },
            DocInnerData::EnumDoc(..) => {
                vec![DocType::Function,
                     DocType::Variant]
            },
            _  => vec![]
        };

        categories.iter().map(|c| self.subitems_in_category(c))
            .filter(|c| c.is_some())
            .map(|c| c.unwrap())
            .collect::<Vec<String>>().join("\n\n")
    }

    fn subitems_in_category(&self, type_: &DocType) -> Option<String> {
        if let Some(items) = self.links.get(type_) {
            if items.len() > 0 {
                let category_str = type_.to_string();
                let items_str = items.iter().cloned().map(|i| i.name ).collect::<Vec<String>>().join("\n");
                Some(format!("==== {}\n{}", category_str, items_str))
            } else {
                None
            }
        } else {
            None
        }
    }

    fn trait_item(&self, item: &TraitItem) -> String {
        let item_string = match item.node {
            TraitItemKind::Const(ref ty, ref expr) => {
                let expr_string = match *expr {
                    Some(ref e) => e.clone(),
                    None    => "".to_string(),
                };
                format!("const {}: {} = {}", self.name, ty.name, expr_string)
            },
            TraitItemKind::Method(ref sig) => {
                format!("fn {} {}", self.name, sig.header)
            },
            TraitItemKind::Type(ref ty) => {
                let ty_string = match *ty {
                    Some(ref t) => t.name.clone(),
                    None    => "".to_string(),
                };
                format!("type {}", ty_string)
            },
            TraitItemKind::Macro(ref mac) => {
                format!("macro {} {}", self.name, mac)
            },
        };
        let doc = self.attrs.doc_strings.join("\n");
        format!("  {}\n{}", item_string, doc)
    }
}

#[derive(Clone, Hash, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub struct DocLink
{
    pub name: String,
    pub path: ModPath,
}

#[derive(Hash, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub enum DocType {
    Function,
    Module,
    Enum,
    Variant,
    Struct,
    StructField,
    Const,
    Trait,
    TraitItemConst,
    TraitItemMethod,
    TraitItemType,
    TraitItemMacro,
}

impl Display for DocType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let name = match *self {
            DocType::Function => "Functions",
            DocType::Module => "Modules",
            DocType::Enum => "Enums",
            DocType::Variant => "Variants",
            DocType::Struct => "Structs",
            DocType::StructField => "Struct Fields",
            DocType::Const => "Constants",
            DocType::Trait => "Traits",
            DocType::TraitItemConst  => &"Associated Constants",
            DocType::TraitItemMethod => &"Trait Methods",
            DocType::TraitItemType   => &"Associated Types",
            DocType::TraitItemMacro  => &"Macros",
        };
        write!(f, "{}", name)
    }
}

/// Describes all possible types of documentation.
#[derive(Eq, PartialEq, Debug, Serialize, Deserialize)]
pub enum DocInnerData {
    FnDoc(Function),
    ModuleDoc(Module),
    EnumDoc(Enum),
    StructDoc(Struct),
    ConstDoc(Constant),
    //StaticDoc,
    //Union,
    //TypedefDoc,
    TraitDoc(Trait),
    TraitItemDoc(TraitItem),
}

impl DocInnerData {
    fn get_doc_file_prefix(&self) -> &str {
        match *self {
            DocInnerData::ModuleDoc(..) => "mdesc-",
            DocInnerData::EnumDoc(..)   => "edesc-",
            DocInnerData::StructDoc(..) => "sdesc-",
            DocInnerData::ConstDoc(..)  => "cdesc-",
            DocInnerData::TraitDoc(..)  => "tdesc-",
            DocInnerData::FnDoc(..) |
            DocInnerData::TraitItemDoc(..) => "",
        }
    }
}
