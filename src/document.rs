use serde::ser::{Serialize};
use serde::de::{Deserialize};
use std::fs::{File, create_dir_all};
use std::io::{Read, Write};
use std::collections::HashMap;
use serde_json;
use std::fmt::{self, Display};
use std::path::PathBuf;
use syntax::ast::{self, Name};
use syntax::abi;
use syntax::codemap::{Span};
use syntax::print::pprust;
use paths;
use store::Store;

use ::errors::*;

#[derive(Eq, PartialEq, Debug, Serialize, Deserialize)]
enum Selector {
    OnModule,
    OnTrait,
    OnStruct,
}

/// Defines a path and identifier for a documentation item, as well as if it belongs to a struct, trait, or directly under a module.
#[derive(Eq, PartialEq, Debug, Serialize, Deserialize)]
pub struct DocSig {
    pub scope: Option<ModPath>,
    //pub selector: Option<Selector>,
    pub identifier: String,
}

impl Display for DocSig {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut scope = match self.scope {
            Some(ref scope) => scope.clone(),
            None => ModPath(Vec::new())
        };
        scope.push(PathSegment{identifier: self.identifier.clone()});

        write!(f, "{}", scope.to_string())
    }
}

// There are redundant enums because we can't derive Serialize/Deserialize on ast's types.
#[derive(Eq, PartialEq, Debug, Serialize, Deserialize)]
pub enum Unsafety {
    Unsafe,
    Normal,
}

impl From<ast::Unsafety> for Unsafety {
    fn from(uns: ast::Unsafety) -> Unsafety {
        match uns {
            ast::Unsafety::Normal => Unsafety::Normal,
            ast::Unsafety::Unsafe => Unsafety::Unsafe,
        }
    }
}

#[derive(Eq, PartialEq, Debug, Serialize, Deserialize)]
pub enum Constness {
    Const,
    NotConst,
}

impl From<ast::Constness> for Constness {
    fn from(con: ast::Constness) -> Constness {
        match con {
            ast::Constness::Const    => Constness::Const,
            ast::Constness::NotConst => Constness::NotConst,
        }
    }
}

#[derive(Eq, PartialEq, Debug, Serialize, Deserialize)]
pub enum Visibility {
    Public,
    Private,
    Inherited,
}

impl From<ast::Visibility> for Visibility{
    fn from(vis: ast::Visibility) -> Visibility {
        match vis {
            ast::Visibility::Public    => Visibility::Public,
            ast::Visibility::Inherited => Visibility::Inherited,
            _                          => Visibility::Private,
        }
    }
}

#[derive(Eq, PartialEq, Debug, Serialize, Deserialize)]
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

impl From<abi::Abi> for Abi {
    fn from(abi: abi::Abi) -> Abi {
        match abi {
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

#[derive(Eq, PartialEq, Debug, Serialize, Deserialize)]
pub enum FnKind {
    ItemFn,
    Method,
    MethodFromImpl,
    MethodFromTrait,
}

#[derive(PartialEq, Eq, Hash, Clone, Debug, Serialize, Deserialize)]
pub struct PathSegment {
    /// The identifier portion of this path segment.
    /// Only the string part of the identifier should be needed for the doc.
    pub identifier: String,

    // TODO: Type/lifetime parameters attached to this path.
    // pub parameters: Option<P<PathParameters>>,
}

impl Display for PathSegment {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.identifier)
    }
}

#[derive(PartialEq, Eq, Hash, Clone, Debug, Serialize, Deserialize)]
pub struct ModPath(pub Vec<PathSegment>);

impl ModPath {
    pub fn new() -> ModPath {
        ModPath(Vec::new())
    }
    pub fn from_ident(span: Span, ident: ast::Ident) -> ModPath {
        ModPath(
            ast::Path::from_ident(span, ident).segments.iter().map(
                |seg| PathSegment { identifier: pprust::ident_to_string(seg.identifier) }).collect()
        )
    }

    pub fn push(&mut self, seg: PathSegment) {
        self.0.push(seg);
    }

    pub fn pop(&mut self) {
        self.0.pop();
    }

    /// All but the final segment of the path.
    pub fn parent(&self) -> Option<ModPath> {
        let mut n = self.clone();
        n.0.pop();
        if let Some(_) = n.0.iter().next() {
            Some(ModPath(n.0))
        } else {
            None
        }
    }

    /// The final segment of the path.
    pub fn name(&self) -> Option<PathSegment> {
        if let Some(seg) = self.0.iter().last() {
            Some(seg.clone())
        } else {
            None
        }
    }

    pub fn join(first: &ModPath, other: &ModPath) -> ModPath {
        let mut result = first.clone();
        result.0.extend(other.0.iter().cloned());
        result
    }

    pub fn to_path(&self) -> PathBuf {
        PathBuf::from(self.0.iter().fold(String::new(), |res, s| res + &s.identifier.clone() + "/"))
    }
}

impl From<String> for ModPath {
    fn from(s: String) -> ModPath {
        ModPath(s.split("::").map(|s| PathSegment { identifier: s.to_string() }).collect::<Vec<PathSegment>>())
    }
}

impl From<ast::Path> for ModPath {
    fn from(p: ast::Path) -> ModPath {
        ModPath(p.segments.iter().map(|s| PathSegment { identifier: pprust::ident_to_string(s.identifier) }).collect::<Vec<PathSegment>>())
    }
}

impl Display for ModPath {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let s = self.0.iter().map(|i| i.identifier.clone()).collect::<Vec<String>>().join("::");

        write!(f, "{}", s)
    }
}

/// Holds the name and version of crate for generating doc directory name
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Package {
    pub name: String,
    pub version: String,
}

/// Holds the TOML fields we care about when deserializing
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CrateInfo {
    pub package: Package,
}

impl Display for CrateInfo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}-{}", self.package.name, self.package.version)
    }
}

pub trait Documentable {
    fn get_info(&self, path: &ModPath) -> String;
    fn get_filename(name: String) -> String;
}

#[derive(Debug, Serialize, Deserialize)]
pub enum DocItem {
    FnItem(Function),
    StructItem(Struct),
    ModuleItem(Module),
}

#[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub struct Attributes {
    pub docstrings: Vec<String>,
}

impl Attributes {
    pub fn new() -> Attributes {
        Attributes {
            docstrings: Vec::new(),
        }
    }

    pub fn from_ast(attrs: &[ast::Attribute]) -> Attributes {
        let mut doc_strings = vec![];
        let mut sp = None;
        let other_attrs: Vec<ast::Attribute> = attrs.iter().filter_map(|attr| {
            attr.with_desugared_doc(|attr| {
                if let Some(value) = attr.value_str() {
                    if attr.check_name("doc") {
                        doc_strings.push(value.to_string());
                        if sp.is_none() {
                            sp = Some(attr.span);
                        }
                        return None;
                    }
                }

                Some(attr.clone())
            })
        }).collect();
        Attributes {
            docstrings: doc_strings,
            //other_attrs: other_attrs,
        }
    }

    /// Finds the `doc` attribute as a NameValue and returns the corresponding
    /// value found.
    pub fn doc_value<'a>(&'a self) -> Option<&'a str> {
        self.docstrings.first().map(|s| &s[..])
    }
}

/// Defines a single documentation item that can be drawn.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Document<T: Documentable> {
    /// Documentation information specific to the type being documented.
    /// Functions have ABI, unsafety, etc. while modules can contain references to other documentation.
    pub doc: T,

    /// Information about the crate the documentation resides in.
    /// Redundant.
    pub crate_info: CrateInfo,

    /// The complete path to this documentation item.
    /// For example, inside crate "krate", module "module", the path for a function "some_fn" is:
    /// ModPath::from("krate::module::some_fn");
    pub path: ModPath,

    /// The one-line overview of the documentation. It is the function signature for functions, "mod module" for modules, etc.
    pub signature: String,

    pub attrs: Attributes,
}

impl<T: Documentable + Serialize + Deserialize> Document<T> {
    fn render(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let doc = match self.attrs.doc_value() {
            Some(d) => d,
            None    => "",
        };
        write!(f, r#"
(from crate {})
=== {}
------------------------------------------------------------------------------
  {}

------------------------------------------------------------------------------

{}
"#, self.crate_info, self.doc.get_info(&self.path), self.signature, doc)
    }

    /// Get the output filename for saving this Document to disk, excluding path.
    pub fn get_filename(&self) -> Result<String> {
        let docfile = paths::encode_doc_filename(&self.path.name().unwrap().identifier)
            .chain_err(|| "Could not encode doc filename")?;

        Ok(T::get_filename(docfile))
    }

    /// Get the complete path to a documentation file, given the path to the store it resides in.
    fn get_docfile(&self, store_path: &PathBuf) -> Result<PathBuf> {
        let parent = self.path.parent();

        let name = self.get_filename()
            .chain_err(|| "Could not resolve documentation filename")?;

        let doc_path = match parent {
            Some(par) => {
                let local_path = par.to_path().join(name);
                store_path.join(local_path)
            }
            None => {
                // TODO: Crates need to be handled seperately.
                // This is the crate's documentation, which lives directly under the store path.
                store_path.join(name)
            }
        };

        Ok(doc_path)
    }

    /// Writes a .odoc JSON store to disk.
    pub fn save_doc(&self, path: &PathBuf) -> Result<PathBuf> {
        let json = serde_json::to_string(&self).unwrap();

        let outfile = self.get_docfile(path)
            .chain_err(|| format!("Could not obtain docfile path inside {}", path.display()))?;

        create_dir_all(outfile.parent().unwrap())
            .chain_err(|| format!("Failed to create module dir {}", path.display()))?;

        let mut fp = File::create(&outfile)
            .chain_err(|| format!("Could not write function odoc file {}", outfile.display()))?;
        fp.write_all(json.as_bytes())
            .chain_err(|| format!("Failed to write to function odoc file {}", outfile.display()))?;

        // Insert the module name into the list of known module names

        info!("Wrote doc to {}", &outfile.display());

        Ok(outfile)
    }

    /// Attempt to load the documentation for a fully qualified documentation path inside the given store path.
    pub fn load_doc(store_path: PathBuf, doc_path: &ModPath) -> Result<Self> {
        let identifier = doc_path.name().unwrap().identifier.clone();
        let encoded_name = paths::encode_doc_filename(&identifier)
            .chain_err(|| "Could not encode doc filename")?;

        let full_path = match doc_path.parent() {
            Some(scope) => {
                store_path.join(scope.to_path())
                    .join(T::get_filename(encoded_name))
            }
            None => {
                store_path.join(T::get_filename(encoded_name))
            }
        };

        info!("Attempting to load doc at {}", &full_path.display());

        let mut fp = File::open(&full_path)
            .chain_err(|| format!("Couldn't find oxidoc store {}", full_path.display()))?;

        let mut json = String::new();
        fp.read_to_string(&mut json)
            .chain_err(|| format!("Couldn't read oxidoc store {}", full_path.display()))?;

        info!("Loading {}", full_path.display());
        let doc: Self = serde_json::from_str(&json)
            .chain_err(|| "Couldn't parse oxidoc store (regen probably needed)")?;

        Ok(doc)
    }
}

impl<T: Documentable + Serialize + Deserialize> Display for Document<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.render(f)
    }
}

#[derive(Eq, PartialEq, Debug, Serialize, Deserialize)]
pub struct StructField {
    type_: String,
    name: Option<String>,
}

impl From<ast::StructField> for StructField {
    fn from(field: ast::StructField) -> StructField {
        let name = match field.ident {
            Some(ident) => Some(pprust::ident_to_string(ident)),
            None       => None,
        };
        StructField {
            type_: pprust::to_string(|s| s.print_type(&field.ty)),
            name: name,
        }
    }
}

impl StructField {
    pub fn from_variant_data(fields: &[ast::StructField]) -> Vec<StructField> {
        fields.iter().cloned().map(|vd| StructField::from(vd)).collect()
    }
}

#[derive(Eq, PartialEq, Debug, Serialize, Deserialize)]
pub struct Struct {
    pub name: String,
    pub fields: Vec<StructField>,
    pub attrs: Attributes,
}

#[derive(Eq, PartialEq, Debug, Serialize, Deserialize)]
pub struct Function {
    pub name: Option<String>,
    pub unsafety: Unsafety,
    pub constness: Constness,
    // TODO: Generics
    pub visibility: Visibility,
    pub abi: Abi,
    pub ty: FnKind,
    pub attrs: Attributes,
}

#[derive(Eq, PartialEq, Debug, Serialize, Deserialize)]
pub struct Module {
    pub name: Option<String>,
    pub structs: Vec<Struct>,
    pub fns: Vec<Function>,
    pub mods: Vec<Module>,
    pub consts: Vec<Constant>,
    pub enums: Vec<Enum>,
    pub impls: Vec<Impl>,
    pub traits: Vec<Trait>,
    pub def_traits: Vec<DefaultImpl>,
    pub is_crate: bool,
    pub attrs: Attributes,
}

impl Module {
    pub fn new(name: Option<String>) -> Module {
        Module {
            name:       name,
            attrs:      Attributes::new(),
            structs:    Vec::new(),
            fns:        Vec::new(),
            mods:       Vec::new(),
            consts:     Vec::new(),
            enums:      Vec::new(),
            impls:      Vec::new(),
            traits:     Vec::new(),
            def_traits: Vec::new(),
            is_crate:   false,
        }
    }
}

#[derive(Eq, PartialEq, Debug, Serialize, Deserialize)]
pub struct Trait {
    pub unsafety: Unsafety,
    pub name: String,
    pub attrs: Attributes,
}

#[derive(Eq, PartialEq, Debug, Serialize, Deserialize)]
pub struct Enum {
    pub variants: Vec<Variant>,
    pub attrs: Attributes,
    pub name: Option<String>,
}

#[derive(Eq, PartialEq, Debug, Serialize, Deserialize)]
pub struct Variant {
    pub name: String,
    pub attrs: Attributes,
    pub data: VariantData,
}

impl From<ast::Variant> for Variant {
       fn from(variant: ast::Variant) -> Variant {
           Variant {
               name: pprust::ident_to_string(variant.node.name),
               attrs: Attributes::from_ast(&variant.node.attrs),
               data: VariantData::from(variant.node.data),
           }
       }
}

#[derive(Eq, PartialEq, Debug, Serialize, Deserialize)]
pub enum VariantData {
    Struct(Vec<StructField>),
    Tuple(Vec<StructField>),
    Unit,
}

impl From<ast::VariantData> for VariantData {
       fn from(variant: ast::VariantData) -> VariantData {
           match variant {
               ast::VariantData::Struct(fields, _) => {
                   let my_fields = StructField::from_variant_data(&fields);
                   VariantData::Struct(my_fields)
               }
               ast::VariantData::Tuple(fields, _) => {
                   let my_fields = StructField::from_variant_data(&fields);
                   VariantData::Tuple(my_fields)
               }
               ast::VariantData::Unit(_) => {
                   VariantData::Unit
               }
           }
       }
}

#[derive(Eq, PartialEq, Debug, Serialize, Deserialize)]
pub struct Constant {
    pub type_: String,
    pub expr: String,
    pub name: String,
    pub attrs: Attributes,
}

#[derive(Eq, PartialEq, Debug, Serialize, Deserialize)]
pub struct Impl {
    pub unsafety: Unsafety,
    //pub generics: ast::Generics,
    pub trait_: Option<TraitRef>,
    pub for_: Ty,
    pub items: Vec<ImplItem>,
    pub attrs: Attributes,
}

#[derive(Eq, PartialEq, Debug, Serialize, Deserialize)]
pub struct DefaultImpl {
    pub unsafety: Unsafety,
    pub trait_: TraitRef,
    pub attrs: Attributes,
}

#[derive(Eq, PartialEq, Debug, Serialize, Deserialize)]
pub struct TraitRef {
    pub path: ModPath,
}

impl From<ast::TraitRef> for TraitRef {
       fn from(trait_ref: ast::TraitRef) -> TraitRef {
           TraitRef {
               path: ModPath::from(trait_ref.path)
           }
       }
}

#[derive(Eq, PartialEq, Debug, Serialize, Deserialize)]
pub struct ImplItem {
    
}

#[derive(Eq, PartialEq, Debug, Serialize, Deserialize)]
pub struct Ty {
    ty: String,
}

impl From<ast::Ty> for Ty {
    fn from(ty: ast::Ty) -> Ty {
        Ty {
            ty: pprust::to_string(|s| s.print_type(&ty)),
        }
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
struct NewDocTemp_ {
    name: String,
    docstring: Option<String>,
    mod_path: ModPath,
    inner_data: DocType,
    // source code reference
    // References to other documents
    links: HashMap<DocType, Vec<DocLink>>,
}

impl NewDocTemp_ {
    fn get_doc_filename(&self) -> String {
        let prefix = self.inner_data.get_doc_file_prefix();
        format!("{}{}.odoc", prefix, self.name)
    }

    fn render(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, r#"
(crate info)
=== doc info
------------------------------------------------------------------------------
  {}

------------------------------------------------------------------------------

{}

{}
"#, self.name, "(docstring)", "(references)")
    }
}

struct DocLink {
    name: String,
    link_path: ModPath,
}

/// Describes all possible types of documentation.
#[derive(Eq, PartialEq, Debug, Serialize, Deserialize)]
enum DocType {
    FnDoc(Function),
    ModuleDoc(Module),
    EnumDoc(Enum),
    StructDoc(Struct),
    ConstDoc(Constant),
    //StaticDoc,
    // Union,
    //TypedefDoc,
    TraitDoc(Trait),
}

impl DocType {
    fn get_doc_file_prefix(&self) -> String {
        match *self {
            DocType::ModuleDoc(..) => "mdesc-",
            DocType::EnumDoc(..)   => "edesc-",
            DocType::StructDoc(..) => "sdesc-",
            DocType::ConstDoc(..)  => "cdesc-",
            DocType::TraitDoc(..)  => "tdesc-",
            DocType::FnDoc(..) |
            _             => "",
        }.to_string()
    }
}
