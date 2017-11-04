use std::collections::HashMap;
use std::fmt::{self, Display};
use std::path::PathBuf;
use std::{slice, vec};

use syntax::ast;
use syntax::abi;
use syntax::codemap::{Span};
use syntax::print::pprust;

use document::ModPath;

// FIXME: Duplication from librustdoc
pub struct ListAttributesIter<'a> {
    attrs: slice::Iter<'a, ast::Attribute>,
    current_list: vec::IntoIter<ast::NestedMetaItem>,
    name: &'a str
}

impl<'a> Iterator for ListAttributesIter<'a> {
    type Item = ast::NestedMetaItem;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(nested) = self.current_list.next() {
            return Some(nested);
        }

        for attr in &mut self.attrs {
            if let Some(list) = attr.meta_item_list() {
                if attr.check_name(self.name) {
                    self.current_list = list.into_iter();
                    if let Some(nested) = self.current_list.next() {
                        return Some(nested);
                    }
                }
            }
        }

        None
    }
}

pub trait AttributesExt {
    /// Finds an attribute as List and returns the list of attributes nested inside.
    fn lists<'a>(&'a self, name: &'a str) -> ListAttributesIter<'a>;
}

impl AttributesExt for [ast::Attribute] {
    fn lists<'a>(&'a self, name: &'a str) -> ListAttributesIter<'a> {
        ListAttributesIter {
            attrs: self.iter(),
            current_list: Vec::new().into_iter(),
            name,
        }
    }
}

pub trait NestedAttributesExt {
    /// Returns whether the attribute list contains a specific `Word`
    fn has_word(self, word: &str) -> bool;
}

impl<I: IntoIterator<Item=ast::NestedMetaItem>> NestedAttributesExt for I {
    fn has_word(self, word: &str) -> bool {
        self.into_iter().any(|attr| attr.is_word() && attr.check_name(word))
    }
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

#[derive(Eq, PartialEq, Clone, Debug, Serialize, Deserialize)]
pub enum FnKind {
    ItemFn,
    MethodFromImpl,
    MethodFromTrait,
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

