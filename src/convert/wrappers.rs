use serde::ser::{Serialize};
use serde::de::{Deserialize};

use convert::doc_containers::*;

#[derive(Eq, PartialEq, Debug, Serialize, Deserialize)]
pub struct Generics {

}

#[derive(Eq, PartialEq, Debug, Serialize, Deserialize)]
pub struct VariantStruct {
    pub fields: DocRelatedItems,
}

#[derive(Eq, PartialEq, Debug, Serialize, Deserialize)]
pub struct Enum {
    pub variants: DocRelatedItems,
}

#[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub struct MethodSig {
    pub unsafety: Unsafety,
    pub constness: Constness,
    pub abi: Abi,
    pub header: String,
}

// There are redundant enums because it isn't possible to derive
// Serialize/Deserialize on ast's types.
#[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub enum Unsafety {
    Unsafe,
    Normal,
}

#[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub enum Constness {
    Const,
    NotConst,
}

#[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub enum Visibility {
    Public,
    Private,
    Inherited,
}

#[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub enum Abi {
    // Single platform ABIs
    Cdecl,
    Stdcall,
    Fastcall,
    Vectorcall,
    Aapcs,
    Win64,
    SysV64,
    PtxKernel,
    Msp430Interrupt,

    // Multiplatform / generic ABIs
    Rust,
    C,
    System,
    RustIntrinsic,
    RustCall,
    PlatformIntrinsic,
    Unadjusted
}

#[derive(Eq, PartialEq, Debug, Serialize, Deserialize)]
pub struct Struct {
    pub fields: DocRelatedItems,
}

#[derive(Eq, PartialEq, Debug, Serialize, Deserialize)]
pub struct Module {
    pub is_crate: bool,
}

#[derive(Eq, PartialEq, Debug, Serialize, Deserialize)]
pub struct Constant {
    pub type_: String,
    pub expr: String,
}

#[derive(Eq, PartialEq, Debug, Serialize, Deserialize)]
pub struct Function {
    pub header: String,
    pub generics: Generics,
    pub unsafety: Unsafety,
    pub constness: Constness,
    pub abi: Abi,
}

#[derive(Eq, PartialEq, Debug, Serialize, Deserialize)]
pub struct Trait {
    pub unsafety: Unsafety,
    // pub generics: Generics,
    // pub bounds: Vec<TyParamBound>,
}

#[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub struct TraitItem {
    pub node: TraitItemKind,
}

#[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub enum TraitItemKind {
    Const(String, Option<String>),
    Method(MethodSig),
    Type(Option<String>),
    Macro(String),
}

impl TraitItemKind {
    pub fn get_category_string(&self) -> &str {
        match *self {
            TraitItemKind::Const(..)  => &"const",
            TraitItemKind::Method(..) => &"fn",
            TraitItemKind::Type(..)   => &"type",
            TraitItemKind::Macro(..)  => &"macro",
        }
    }
}
