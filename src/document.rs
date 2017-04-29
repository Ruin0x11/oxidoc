use std::fs::{File, create_dir_all};
use std::io::{Read, Write};
use std::collections::HashMap;
use std::fmt::{self, Display};
use std::path::PathBuf;

use syntax::ast::{self, Name};
use syntax::abi;
use syntax::codemap::{Span};
use syntax::print::pprust;
use paths;
use store::Store;

use ::errors::*;

#[derive(Eq, PartialEq, Clone, Debug, Serialize, Deserialize)]
pub enum FnKind {
    ItemFn,
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

    pub fn append_ident(&self, ident: ast::Ident) -> ModPath {
        let mut path = self.clone();
        let name = pprust::ident_to_string(ident);
        path.push_string(name);
        path
    }


    pub fn push(&mut self, seg: PathSegment) {
        self.0.push(seg);
    }

    pub fn push_string(&mut self, s: String) {
        self.0.push(PathSegment { identifier: s });
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

    pub fn head(&self) -> Option<PathSegment> {
        if let Some(seg) = self.0.iter().next() {
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

    pub fn to_filepath(&self) -> PathBuf {
        PathBuf::from(self.0.iter().fold(String::new(), |res, s| res + &s.identifier.clone() + "/"))
    }
}

impl From<String> for ModPath {
    fn from(s: String) -> ModPath {
        ModPath(s.split("::").map(|s| PathSegment { identifier: s.to_string() }).collect::<Vec<PathSegment>>())
    }
}

impl From<ast::Ident> for ModPath {
    fn from(i: ast::Ident) -> ModPath {
        ModPath::from(pprust::ident_to_string(i))
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

#[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub struct Attributes {
    pub doc_strings: Vec<String>,
}

impl Attributes {
    pub fn new() -> Attributes {
        Attributes {
            doc_strings: Vec::new(),
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
            doc_strings: doc_strings,
            //other_attrs: other_attrs,
        }
    }

    /// Finds the `doc` attribute as a NameValue and returns the corresponding
    /// value found.
    pub fn doc_value<'a>(&'a self) -> Option<&'a str> {
        self.doc_strings.first().map(|s| &s[..])
    }
}

#[derive(Clone, Debug)]
pub struct StructField {
    type_: ast::Ty,
    name: Option<String>,
}

#[derive(Clone, Debug)]
pub struct Struct {
    pub ident: ast::Ident,
    pub id: NodeId,
    pub vis: ast::Visibility,
    pub fields: Vec<ast::StructField>,
    pub attrs: Vec<ast::Attribute>,
    pub path: ModPath,
}

#[derive(Clone, Debug)]
pub struct Function {
    pub ident: ast::Ident,
    pub unsafety: ast::Unsafety,
    pub constness: ast::Constness,
    pub decl: ast::FnDecl,
    // TODO: Generics
    pub vis: ast::Visibility,
    pub abi: abi::Abi,
    pub attrs: Vec<ast::Attribute>,
    pub kind: FnKind,
    pub path: ModPath,
}

#[derive(Clone, Debug)]
pub struct Module {
    pub ident: Option<ast::Ident>,
    pub vis: ast::Visibility,
    pub imports: Vec<Import>,
    pub structs: Vec<Struct>,
    pub fns: Vec<Function>,
    pub mods: Vec<Module>,
    pub consts: Vec<Constant>,
    pub enums: Vec<Enum>,
    pub impls: Vec<Impl>,
    pub traits: Vec<Trait>,
    pub def_traits: Vec<DefaultImpl>,
    pub is_crate: bool,
    pub attrs: Vec<ast::Attribute>,
    pub path: ModPath,

    /// A mapping from identifers that are 'use'd within this module to the full
    /// namespace they resolve to.
    pub namespaces_to_paths: HashMap<String, ModPath>,
}

impl Module {
    pub fn new(ident: Option<ast::Ident>) -> Module {
        Module {
            ident:      ident,
            vis:        ast::Visibility::Inherited,
            attrs:      Vec::new(),
            imports:    Vec::new(),
            structs:    Vec::new(),
            fns:        Vec::new(),
            mods:       Vec::new(),
            consts:     Vec::new(),
            enums:      Vec::new(),
            impls:      Vec::new(),
            traits:     Vec::new(),
            def_traits: Vec::new(),
            is_crate:   false,
            path:       ModPath::new(),
            namespaces_to_paths: HashMap::new(),
        }

    }

    pub fn add_use(&mut self,
               ident: &ast::Ident,
               path: ModPath) {
        let identifier = pprust::ident_to_string(*ident);
        let namespace = ModPath::from(path.clone());
        self.namespaces_to_paths.insert(identifier, namespace);
    }

    pub fn resolve_use(&self, namespaced_path: &ModPath) -> Option<ModPath> {
        let ident = namespaced_path.head()
            .expect("Given path was empty!").identifier;
        println!("");
        match self.namespaces_to_paths.get(&ident) {
            Some(u) => Some(ModPath::join(&u.parent().expect("Found empty 'use' namespace in module!"), &namespaced_path)),
            None    => None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Trait {
    pub items: Vec<TraitItem>,
    pub ident: ast::Ident,
    pub unsafety: ast::Unsafety,
    pub vis: ast::Visibility,
    pub attrs: Vec<ast::Attribute>,
    pub path: ModPath,
}
#[derive(Clone, Debug)]
pub struct TraitItem {
    pub ident: ast::Ident,
    pub attrs: Vec<ast::Attribute>,
    pub path: ModPath,
    pub node: ast::TraitItemKind,
}
#[derive(Clone, Debug)]
pub struct Enum {
    pub ident: ast::Ident,
    pub vis: ast::Visibility,
    pub variants: Vec<ast::Variant>,
    pub attrs: Vec<ast::Attribute>,
    pub path: ModPath,
}

#[derive(Clone, Debug)]
pub struct Variant {
    pub ident: ast::Ident,
    pub attrs: Vec<ast::Attribute>,
    pub data: ast::VariantData,
    pub path: ModPath,
}

#[derive(Clone, Debug)]
pub struct Constant {
    pub type_: Ty,
    pub expr: ast::Expr,
    pub ident: ast::Ident,
    pub vis: ast::Visibility,
    pub attrs: Vec<ast::Attribute>,
    pub path: ModPath,
}

#[derive(Clone, Debug)]
pub struct Impl {
    pub unsafety: ast::Unsafety,
    //pub generics: ast::Generics,
    pub trait_: Option<ast::TraitRef>,
    pub for_: ast::Ty,
    pub items: Vec<ast::ImplItem>,
    pub attrs: Vec<ast::Attribute>,
    pub path: ModPath,
}

#[derive(Clone, Debug)]
pub struct DefaultImpl {
    pub unsafety: ast::Unsafety,
    pub trait_: ast::TraitRef,
    pub attrs: Vec<ast::Attribute>,
}

#[derive(Clone, Debug)]
pub struct Import {
    pub path: ast::ViewPath,
}

// These structs have importance in the initial AST visit, because all impls for
// types have to be resolved by conversion. There could be types whose
// implementation lives in another module.

#[derive(Hash, Eq, PartialEq, Clone, Debug, Serialize, Deserialize)]
pub struct NodeId(u32);

impl From<ast::NodeId> for NodeId {
    fn from(id: ast::NodeId) -> NodeId {
        NodeId(id.as_u32())
    }
}

#[derive(Hash, Eq, PartialEq, Clone, Debug, Serialize, Deserialize)]
pub struct Ty {
    pub id: NodeId,
    pub name: String,
}

impl From<ast::Ty> for Ty {
    fn from(ty: ast::Ty) -> Self{
        Ty {
            id: NodeId::from(ty.id),
            name: pprust::ty_to_string(&ty),
        }
    }
}
