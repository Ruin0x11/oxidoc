use store::StoreLocation;
use std::collections::HashMap;
use std::fs;
use std::fmt::{self, Display};

use document::{Attributes, CrateInfo, ModPath};
use store;

use convert::wrappers::*;
use convert::wrappers::TraitItemKind;
use ::errors::*;

pub use self::DocInnerData::*;

pub type DocRelatedItems = HashMap<DocType, Vec<DocLink>>;

#[derive(Eq, PartialEq, Debug, Serialize, Deserialize)]
pub struct NewDocTemp_ {
    pub name: String,
    pub attrs: Attributes,
    pub crate_info: CrateInfo,
    pub mod_path: ModPath,
    pub inner_data: DocInnerData,
    pub visibility: Option<Visibility>,
    // TODO: source code reference
    pub links: DocRelatedItems,
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
    pub fn get_type(&self) -> DocType {
        match self.inner_data {
            DocInnerData::FnDoc(..) => {
                DocType::Function
            },
            DocInnerData::ModuleDoc(..) => {
                DocType::Module
            },
            DocInnerData::EnumDoc(..) => {
                DocType::Enum
            },
            DocInnerData::StructDoc(..) => {
                DocType::Struct
            },
            DocInnerData::ConstDoc(..) => {
                DocType::Const
            },
            DocInnerData::TraitDoc(..) => {
                DocType::Trait
            },
            DocInnerData::TraitItemDoc(ref item) => {
                    match item.node {
                        TraitItemKind::Const(..)  => DocType::TraitItemConst,
                        TraitItemKind::Method(..) => DocType::TraitItemMethod,
                        TraitItemKind::Type(..)   => DocType::TraitItemType,
                        TraitItemKind::Macro(..)  => DocType::TraitItemMacro,
                    }
            },

        }
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

    pub fn to_store_location(&self) -> StoreLocation {
        StoreLocation {
            name: self.name.clone(),
            crate_info: self.crate_info.clone(),
            mod_path: self.mod_path.clone(),
            doc_type: self.get_type(),
        }
    }

    pub fn save(&self) -> Result<()> {
        let location = self.to_store_location();
        let path = location.to_filepath();

        {
            let parent_path = path.parent().unwrap();

            fs::create_dir_all(parent_path)
                .chain_err(|| format!("Failed to create directory {}", parent_path.display()))?;
        }

        store::serialize_object(self, path)
    }
}

#[derive(Clone, Hash, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub struct DocLink
{
    pub name: String,
    pub path: ModPath,
}

#[derive(Clone, Hash, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub enum DocType {
    Function,
    // Method,
    Module,
    Enum,
    Variant,
    Struct,
    StructField,
    Const,
    Trait,
    AssocConst,
    TraitItemMethod,
    TraitItemConst,
    TraitItemType,
    TraitItemMacro,
    AssocType,
    Macro,
}

impl DocType {
    pub fn get_file_prefix(&self) -> &str {
        match *self {
            DocType::Function => "",
            DocType::Module => "mdesc-",
            DocType::Enum => "edesc-",
            DocType::Variant => "vdesc-",
            DocType::Struct => "sdesc-",
            DocType::StructField => "sfdesc-",
            DocType::Const => "cdesc-",
            DocType::Trait => "tdesc-",
            DocType::AssocConst  => &"acdesc-",
            DocType::TraitItemConst => &"tcdesc-",
            DocType::TraitItemMethod => &"tmcdesc-",
            DocType::TraitItemType => &"ttcdesc-",
            DocType::TraitItemMacro => &"tmdesc-",
            DocType::AssocType   => &"atdesc-",
            DocType::Macro  => &"macdesc-",
        }
    }
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
            DocType::TraitItemConst => &"Trait Constants",
            DocType::TraitItemMethod => &"Trait Methods",
            DocType::TraitItemType => &"Trait Types",
            DocType::TraitItemMacro => &"Trait Macros",
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
