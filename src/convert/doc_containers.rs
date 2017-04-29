use std::collections::HashMap;
use std::path::PathBuf;
use std::fmt::{self, Display};

use serde::ser::{Serialize};
use serde::de::{Deserialize};
use syntax::abi;
use syntax::ast;
use syntax::symbol::keywords;
use syntax::print::pprust;
use syntax::ptr::P;

use document::{self, FnKind, Attributes, CrateInfo, PathSegment, ModPath};
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
    // TODO: source code reference
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
            DocInnerData::FnDoc(ref func) => {
                match func.kind {
                    FnKind::MethodFromImpl => format!("=== Impl on type {}", self.mod_path.parent().unwrap()),
                    _ => format!("=== In module {}", self.mod_path.parent().unwrap()),
                }
            },
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
                vec![DocType::AssocConst,
                     DocType::TraitItemMethod,
                     DocType::AssocType,
                     DocType::Macro]
            },
            DocInnerData::StructDoc(..) => {
                vec![DocType::StructField,
                     DocType::Function,
                     DocType::AssocConst,
                     DocType::AssocType,
                     DocType::Macro]
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

    pub fn to_filepath(&self) -> PathBuf {
        let mut path = self.mod_path.to_filepath();
        path.push(self.get_doc_filename());
        let prefix = PathBuf::from("{{root}}");
        let stripped = path.strip_prefix(&prefix).unwrap();
        stripped.to_path_buf()
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
    AssocConst,
    TraitItemMethod,
    AssocType,
    Macro,
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
            DocType::AssocConst  => &"Associated Constants",
            DocType::TraitItemMethod => &"Trait Methods",
            DocType::AssocType   => &"Associated Types",
            DocType::Macro  => &"Macros",
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
