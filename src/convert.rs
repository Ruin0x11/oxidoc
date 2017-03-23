//! Functions to convert the data taken from the AST into documentation.
//! Borrows ideas from librustdoc's Clean.

pub use self::DocInnerData::*;

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

pub struct Context {
    pub store_path: PathBuf,
    pub crate_info: CrateInfo,
}

pub trait Convert<T> {
    fn convert(&self, context: &Context) -> T;
}

impl<T: Convert<U>, U> Convert<Vec<U>> for [T] {
    fn convert(&self, cx: &Context) -> Vec<U> {
        self.iter().map(|x| x.convert(cx)).collect()
    }
}

impl<T: Convert<U>, U> Convert<U> for P<T> {
    fn convert(&self, cx: &Context) -> U {
        (**self).convert(cx)
    }
}

impl<T: Convert<U>, U> Convert<Option<U>> for Option<T> {
    fn convert(&self, cx: &Context) -> Option<U> {
        self.as_ref().map(|v| v.convert(cx))
    }
}

#[derive(Eq, PartialEq, Debug, Serialize, Deserialize)]
pub struct Generics {

}

#[derive(Eq, PartialEq, Debug, Serialize, Deserialize)]
struct Module {
    is_crate: bool,
}

#[derive(Eq, PartialEq, Debug, Serialize, Deserialize)]
struct Function {
    header: String,
    generics: Generics,
    unsafety: Unsafety,
    constness: Constness,
    abi: Abi,
}

#[derive(Eq, PartialEq, Debug, Serialize, Deserialize)]
struct Constant {
    type_: String,
    expr: String,
}

#[derive(Eq, PartialEq, Debug, Serialize, Deserialize)]
struct Struct {
    fields: Vec<NewDocTemp_>,
}

#[derive(Eq, PartialEq, Debug, Serialize, Deserialize)]
struct VariantStruct {
    fields: Vec<NewDocTemp_>,
}

#[derive(Eq, PartialEq, Debug, Serialize, Deserialize)]
struct Enum {
    variants: Vec<NewDocTemp_>,
}

#[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
struct MethodSig {
    unsafety: Unsafety,
    constness: Constness,
    abi: Abi,
    header: String,
}

#[derive(Eq, PartialEq, Debug, Serialize, Deserialize)]
pub struct Trait {
    pub unsafety: Unsafety,
    // pub generics: Generics,
    // pub bounds: Vec<TyParamBound>,
}

#[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub struct TraitItem {
    node: TraitItemKind,
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

// There are redundant enums because it isn't possible to derive
// Serialize/Deserialize on ast's types.
#[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub enum Unsafety {
    Unsafe,
    Normal,
}

impl Convert<Unsafety> for ast::Unsafety {
    fn convert(&self, context: &Context) -> Unsafety {
        match *self {
            ast::Unsafety::Normal => Unsafety::Normal,
            ast::Unsafety::Unsafe => Unsafety::Unsafe,
        }
    }
}

#[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub enum Constness {
    Const,
    NotConst,
}

impl Convert<Constness> for ast::Constness {
    fn convert(&self, context: &Context) -> Constness {
        match *self {
            ast::Constness::Const    => Constness::Const,
            ast::Constness::NotConst => Constness::NotConst,
        }
    }
}

#[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub enum Visibility {
    Public,
    Private,
    Inherited,
}

impl Convert<Visibility> for ast::Visibility{
    fn convert(&self, context: &Context) -> Visibility {
        match *self {
            ast::Visibility::Public    => Visibility::Public,
            ast::Visibility::Inherited => Visibility::Inherited,
            _                          => Visibility::Private,
        }
    }
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

impl Convert<Abi> for abi::Abi {
    fn convert(&self, context: &Context) -> Abi {
        match *self {
            abi::Abi::Cdecl             => Abi::Cdecl,
            abi::Abi::Stdcall           => Abi::Stdcall,
            abi::Abi::Fastcall          => Abi::Fastcall,
            abi::Abi::Vectorcall        => Abi::Vectorcall,
            abi::Abi::Aapcs             => Abi::Aapcs,
            abi::Abi::Win64             => Abi::Win64,
            abi::Abi::SysV64            => Abi::SysV64,
            abi::Abi::PtxKernel         => Abi::PtxKernel,
            abi::Abi::Msp430Interrupt   => Abi::Msp430Interrupt,
            abi::Abi::Rust              => Abi::Rust,
            abi::Abi::C                 => Abi::C,
            abi::Abi::System            => Abi::System,
            abi::Abi::RustIntrinsic     => Abi::RustIntrinsic,
            abi::Abi::RustCall          => Abi::RustCall,
            abi::Abi::PlatformIntrinsic => Abi::PlatformIntrinsic,
            abi::Abi::Unadjusted        => Abi::Unadjusted
        }
    }
}

impl<'a> Convert<Store> for OxidocVisitor<'a> {
    fn convert(&self, context: &Context) -> Store {
        debug!("Converting store");
        let mut store = Store::new(context.store_path.clone());

        let documents = self.crate_module.convert(context);

        for doc in &store.documents {
            debug!("{:?}", doc);
        }

        store.documents = documents;

        store
    }
}

impl Convert<Vec<NewDocTemp_>> for document::Module {
    fn convert(&self, context: &Context) -> Vec<NewDocTemp_> {
        let mut docs: Vec<NewDocTemp_> = vec![];

        docs.extend(self.consts.iter().map(|x| x.convert(context)));
        docs.extend(self.traits.iter().map(|x| x.convert(context)));
        docs.extend(self.fns.iter().map(|x| x.convert(context)));
        docs.extend(self.mods.iter().flat_map(|x| x.convert(context)));

        let name = match self.ident {
            Some(id) => id.convert(context),
            None     => context.crate_info.package.name.clone(),
        };

        let mod_doc = NewDocTemp_ {
            name: name.clone(),
            attrs: self.attrs.convert(context),
            mod_path: self.path.clone(),
            visibility: Some(self.vis.convert(context)),
            inner_data: ModuleDoc(Module {
                is_crate: self.is_crate,
            }),
            links: HashMap::new(),
        };

        docs.push(mod_doc);

        docs
    }
}

impl Convert<NewDocTemp_> for document::Constant {
    fn convert(&self, context: &Context) -> NewDocTemp_ {
        NewDocTemp_ {
            name: self.ident.convert(context),
            attrs: self.attrs.convert(context),
            mod_path: self.path.clone(),
            visibility: Some(self.vis.convert(context)),
            inner_data: ConstDoc(Constant {
                type_: self.type_.convert(context),
                expr: self.expr.convert(context),
            }),
            links: HashMap::new(),
        }
    }
}

impl Convert<NewDocTemp_> for document::Function {
    fn convert(&self, context: &Context) -> NewDocTemp_ {
        NewDocTemp_ {
            name: self.ident.convert(context),
            attrs: self.attrs.convert(context),
            mod_path: self.path.clone(),
            visibility: Some(self.vis.convert(context)),
            inner_data: FnDoc(Function {
                header: self.decl.convert(context),
                generics: Generics { } ,
                unsafety: self.unsafety.convert(context),
                constness: self.constness.convert(context),
                abi: self.abi.convert(context),
            }),
            links: HashMap::new(),
        }
    }
}

impl Convert<HashMap<DocType, Vec<DocLink>>> for [document::TraitItem] {
    fn convert(&self, context: &Context) -> HashMap<DocType, Vec<DocLink>> {
        let mut consts = Vec::new();
        let mut methods = Vec::new();
        let mut types = Vec::new();
        let mut macros = Vec::new();

        for item in self {
            match item.node {
                ast::TraitItemKind::Const(..) => consts.push(item.clone()),
                ast::TraitItemKind::Method(..) => methods.push(item.clone()),
                ast::TraitItemKind::Type(..) => types.push(item.clone()),
                ast::TraitItemKind::Macro(..) => macros.push(item.clone()),
            }
        }

        let conv = |items: Vec<document::TraitItem>| {
            items.iter().cloned().map(|item|
                                      DocLink {
                                          name: item.ident.convert(context),
                                          path: item.path.clone(),
                                      }
            ).collect()
        };

        let consts_n = conv(consts);
        let methods_n = conv(methods);
        let types_n = conv(types);
        let macros_n = conv(macros);

        let mut links = HashMap::new();
        links.insert(DocType::TraitItemConst, consts_n);
        links.insert(DocType::TraitItemMethod, methods_n);
        links.insert(DocType::TraitItemType, types_n);
        links.insert(DocType::TraitItemMacro, macros_n);
        links
    }
}

impl Convert<NewDocTemp_> for document::Trait {
    fn convert(&self, context: &Context) -> NewDocTemp_ {

        NewDocTemp_ {
            name: self.ident.convert(context),
            attrs: self.attrs.convert(context),
            mod_path: self.path.clone(),
            visibility: Some(self.vis.convert(context)),
            inner_data: TraitDoc(Trait {
                unsafety: self.unsafety.convert(context),
            }),
            links: self.items.convert(context),
        }
    }
}

impl Convert<NewDocTemp_> for document::TraitItem {
    fn convert(&self, context: &Context) -> NewDocTemp_ {
        NewDocTemp_ {
            name: self.ident.convert(context),
            attrs: self.attrs.convert(context),
            mod_path: self.path.clone(),
            visibility: Some(Visibility::Inherited),
            inner_data: TraitItemDoc(TraitItem {
                node: self.node.convert(context),
            }),
            links: HashMap::new(),
        }
    }
}

impl Convert<TraitItemKind> for ast::TraitItemKind {
    fn convert(&self, context: &Context) -> TraitItemKind {
        match *self {
            ast::TraitItemKind::Const(ref ty, ref expr) => {
                TraitItemKind::Const(ty.convert(context), expr.convert(context))
            },
            ast::TraitItemKind::Method(ref sig, ref block) => {
                TraitItemKind::Method(sig.convert(context))
            },
            ast::TraitItemKind::Type(ref bounds, ref ty) => {
                TraitItemKind::Type(ty.convert(context))
            },
            ast::TraitItemKind::Macro(ref mac) => {
                TraitItemKind::Macro(mac.convert(context))
            },
        }
    }
}

impl Convert<MethodSig> for ast::MethodSig {
    fn convert(&self, context: &Context) -> MethodSig {
        MethodSig {
            unsafety: self.unsafety.convert(context),
            constness: self.constness.node.convert(context),
            abi: self.abi.convert(context),
            header: self.decl.convert(context),
        }
    }
}

impl Convert<String> for ast::FnDecl {
    fn convert(&self, context: &Context) -> String {
        pprust::to_string(|s| s.print_fn_args_and_ret(self))
    }
}

impl Convert<String> for ast::Ty {
    fn convert(&self, context: &Context) -> String {
        pprust::ty_to_string(self)
    }
}

impl Convert<String> for ast::Expr {
    fn convert(&self, context: &Context) -> String {
        pprust::expr_to_string(self)
    }
}

impl Convert<String> for ast::Ident {
    fn convert(&self, context: &Context) -> String {
        pprust::ident_to_string(*self)
    }
}

impl Convert<String> for ast::Name {
    fn convert(&self, context: &Context) -> String {
        pprust::to_string(|s| s.print_name(*self))
    }
}

impl Convert<String> for ast::Mac {
    fn convert(&self, context: &Context) -> String {
        pprust::mac_to_string(self)
    }
}

impl Convert<Attributes> for [ast::Attribute] {
    fn convert(&self, context: &Context) -> Attributes {
        Attributes::from_ast(self)
    }
}

// -----
// mod crate::the_mod
// -----
// A useful module.
//
// == Functions:
// function_a(), function_b(), function_c()
//
// == Statics:
// STATIC_A, STATIC_B
//
// == Enums:
// CoolEnum, NiceEnum



// == (impl on crate::MyStruct)
// -----
// struct name_of_struct { /* fields omitted */ }
// -----
// Does a thing.
//
// == Functions:
// function_a(), function_b(), function_c()
//
// ==== from trait crate::NiceTrait:
// cool(), sweet(), okay()

// TODO: Testing a new design.
#[derive(Eq, PartialEq, Debug, Serialize, Deserialize)]
pub struct NewDocTemp_ {
    name: String,
    attrs: Attributes,
    mod_path: ModPath,
    inner_data: DocInnerData,
    visibility: Option<Visibility>,
    // source code reference
    // References to other documents
    links: HashMap<DocType, Vec<DocLink>>,
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
                format!("=== (in module {})", self.mod_path.parent().unwrap())
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
                format!("const {}: {} = {}", self.name, const_.type_, const_.expr)
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

    fn subitems(&self) -> String {
        let categories = match self.inner_data {
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
            DocInnerData::StructDoc(..) |
            DocInnerData::EnumDoc(..) => {
                vec![DocType::Function]
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
                format!("const {}: {} = {}", self.name, ty, expr_string)
            },
            TraitItemKind::Method(ref sig) => {
                format!("fn {} {}", self.name, sig.header)
            },
            TraitItemKind::Type(ref ty) => {
                let ty_string = match *ty {
                    Some(ref t) => t.clone(),
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
struct DocLink
{
    name: String,
    path: ModPath,
}

#[derive(Hash, Eq, PartialEq, Debug, Serialize, Deserialize)]
enum DocType {
    Function,
    Module,
    Enum,
    Struct,
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
            DocType::Struct => "Structs",
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
enum DocInnerData {
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
    fn get_doc_file_prefix(&self) -> String {
        match *self {
            DocInnerData::ModuleDoc(..) => "mdesc-",
            DocInnerData::EnumDoc(..)   => "edesc-",
            DocInnerData::StructDoc(..) => "sdesc-",
            DocInnerData::ConstDoc(..)  => "cdesc-",
            DocInnerData::TraitDoc(..)  => "tdesc-",
            DocInnerData::FnDoc(..) |
            _             => "",
        }.to_string()
    }
}
