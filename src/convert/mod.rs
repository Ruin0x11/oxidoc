//! Functions to convert the data taken from the AST into documentation.
//! Borrows ideas from librustdoc's Clean.

mod wrappers;
mod doc_containers;

pub use convert::doc_containers::*;

use std::collections::HashMap;
use std::path::PathBuf;

use syntax::abi;
use syntax::ast;
use syntax::print::pprust;
use syntax::ptr::P;

use document::{self, NodeId, Impl, Ty, Attributes, CrateInfo, ModPath};
use store::Store;
use visitor::OxidocVisitor;

use convert::wrappers::*;

#[derive(Clone)]
pub struct Context {
    pub store_path: PathBuf,
    pub crate_info: CrateInfo,
    /// Mapping from types to their implementations. Received from the AST
    /// visitor.
    pub impls_for_ty: HashMap<ModPath, Vec<Impl>>,
}

impl Context {
    pub fn new(store_path: PathBuf,
               crate_info: CrateInfo,
               impls_for_ty: HashMap<ModPath, Vec<Impl>>) -> Self {
        Context {
            store_path: store_path,
            crate_info: crate_info,
            impls_for_ty: impls_for_ty,
        }
    }
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

impl Convert<Unsafety> for ast::Unsafety {
    fn convert(&self, _context: &Context) -> Unsafety {
        match *self {
            ast::Unsafety::Normal => Unsafety::Normal,
            ast::Unsafety::Unsafe => Unsafety::Unsafe,
        }
    }
}

impl Convert<Constness> for ast::Constness {
    fn convert(&self, _context: &Context) -> Constness {
        match *self {
            ast::Constness::Const    => Constness::Const,
            ast::Constness::NotConst => Constness::NotConst,
        }
    }
}

impl Convert<Visibility> for ast::Visibility{
    fn convert(&self, _context: &Context) -> Visibility {
        match *self {
            ast::Visibility::Public    => Visibility::Public,
            ast::Visibility::Inherited => Visibility::Inherited,
            _                          => Visibility::Private,
        }
    }
}

impl Convert<Abi> for abi::Abi {
    fn convert(&self, _context: &Context) -> Abi {
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

impl Convert<Store> for OxidocVisitor {
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

impl Convert<Vec<Documentation>> for document::Module {
    fn convert(&self, context: &Context) -> Vec<Documentation> {
        let mut docs: Vec<Documentation> = vec![];

        for (ident, path) in self.namespaces_to_paths.iter() {
            println!("in {:?}, {} => {}", self.ident, ident, path);
        }

        docs.extend(self.consts.iter().map(|x| x.convert(context)));
        docs.extend(self.traits.iter().map(|x| x.convert(context)));
        docs.extend(self.fns.iter().map(|x| x.convert(context)));
        docs.extend(self.mods.iter().flat_map(|x| x.convert(context)));
        docs.extend(self.structs.iter().map(|x| x.convert(context)));
        // unions
        docs.extend(self.enums.iter().map(|x| x.convert(context)));
        // foreigns
        // typedefs
        // statics
        // macros
        // def_traits

        let name = match self.ident {
            Some(id) => id.convert(context),
            None     => context.crate_info.package.name.clone(),
        };

        let mod_doc = Documentation {
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

impl Convert<Documentation> for document::Constant {
    fn convert(&self, context: &Context) -> Documentation {
        Documentation {
            name: self.ident.convert(context),
            attrs: self.attrs.convert(context),
            mod_path: self.path.clone(),
            visibility: Some(self.vis.convert(context)),
            inner_data: ConstDoc(Constant {
                ty: self.type_.clone(),
                expr: self.expr.convert(context),
            }),
            links: HashMap::new(),
        }
    }
}

impl Convert<Documentation> for document::Function {
    fn convert(&self, context: &Context) -> Documentation {
        Documentation {
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
                kind: self.kind.clone(),
            }),
            links: HashMap::new(),
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

impl Convert<Documentation> for document::Trait {
    fn convert(&self, context: &Context) -> Documentation {
        Documentation {
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

impl Convert<Documentation> for document::TraitItem {
    fn convert(&self, context: &Context) -> Documentation {
        Documentation {
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

impl Convert<DocRelatedItems> for [document::TraitItem] {
    fn convert(&self, context: &Context) -> DocRelatedItems {
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

        let mut links = HashMap::new();
        links.insert(DocType::AssocConst, conv(consts));
        links.insert(DocType::TraitItemMethod, conv(methods));
        links.insert(DocType::AssocType, conv(types));
        links.insert(DocType::Macro, conv(macros));
        links
    }
}

impl Convert<TraitItemKind> for ast::TraitItemKind {
    fn convert(&self, context: &Context) -> TraitItemKind {
        match *self {
            ast::TraitItemKind::Const(ref ty, ref expr) => {
                TraitItemKind::Const(ty.convert(context), expr.convert(context))
            },
            ast::TraitItemKind::Method(ref sig, ref _block) => {
                TraitItemKind::Method(sig.convert(context))
            },
            ast::TraitItemKind::Type(ref _bounds, ref ty) => {
                TraitItemKind::Type(ty.convert(context))
            },
            ast::TraitItemKind::Macro(ref mac) => {
                TraitItemKind::Macro(mac.convert(context))
            },
        }
    }
}

impl Convert<Documentation> for document::Struct {
    fn convert(&self, context: &Context) -> Documentation {
        let mut links: DocRelatedItems = self.fields.convert(context);
        if let Some(impls) = context.impls_for_ty.get(&self.path) {
            for impl_ in impls {
                let impl_links = impl_.convert(context);
                println!("Impl found for {}!", self.path);
                links.extend(impl_links);
            }
        }

        Documentation {
            name: self.ident.convert(context),
            attrs: self.attrs.convert(context),
            mod_path: self.path.clone(),
            visibility: Some(self.vis.convert(context)),
            inner_data: StructDoc(Struct {
                fields: self.fields.convert(context),
            }),
            links: links,
        }
    }
}

impl Convert<DocRelatedItems> for document::Impl {
    fn convert(&self, context: &Context) -> DocRelatedItems {
        let mut consts = Vec::new();
        let mut methods = Vec::new();
        let mut types = Vec::new();
        let mut macros = Vec::new();
        for item in &self.items {
            match item.node {
                ast::ImplItemKind::Const(..)  => consts.push(item.clone()),
                ast::ImplItemKind::Method(..) => methods.push(item.clone()),
                ast::ImplItemKind::Type(..)   => types.push(item.clone()),
                ast::ImplItemKind::Macro(..)  => macros.push(item.clone()),
            }
        }

        let conv = |items: Vec<ast::ImplItem>| {
            items.iter().cloned().map(|item| {
                let name = item.ident.convert(context);
                DocLink {
                    name: name.clone(),
                    path: ModPath::join(&self.path.clone(),
                                        &ModPath::from(name))
                }
            }
            ).collect()
        };

        let mut links = HashMap::new();
        links.insert(DocType::AssocConst, conv(consts));
        links.insert(DocType::Function, conv(methods));
        links.insert(DocType::AssocType, conv(types));
        links.insert(DocType::Macro, conv(macros));
        links
    }
}

impl Convert<DocRelatedItems> for [ast::StructField] {
    fn convert(&self, context: &Context) -> DocRelatedItems {
        let mut fields = Vec::new();

        for item in self {
            if item.ident.is_none() {
                continue;
            }
            let field = item.convert(context);
            let field_link = DocLink {
                // TODO: Display nicely, with signature
                name: field.ident.unwrap(),
                path: field.path.clone(),
            };
            fields.push(field_link);
        }
        let mut links = HashMap::new();
        links.insert(DocType::StructField, fields);
        links
    }
}

impl Convert<StructField> for ast::StructField {
    fn convert(&self, context: &Context) -> StructField {
        StructField {
            ident: self.ident.convert(context),
            vis: self.vis.convert(context),
            ty: self.ty.convert(context),
            attrs: self.attrs.convert(context),
            path: ModPath::new(),
        }
    }
}

impl Convert<Documentation> for document::Enum {
    fn convert(&self, context: &Context) -> Documentation {
        Documentation {
            name: self.ident.convert(context),
            attrs: self.attrs.convert(context),
            mod_path: self.path.clone(),
            visibility: Some(Visibility::Inherited),
            inner_data: EnumDoc(Enum {
                variants: self.variants.convert(context),
            }),
            links: self.variants.convert(context),
        }
    }
}

impl Convert<Ty> for ast::Ty {
    fn convert(&self, _context: &Context) -> Ty {
        Ty::from(self.clone())
    }
}

impl Convert<DocRelatedItems> for [ast::Variant] {
    fn convert(&self, _context: &Context) -> DocRelatedItems {
        let mut variants = Vec::new();

        for item in self {
            // TODO: These are just strings for now, instead of separate docs.
            let variant_link = DocLink {
                name: pprust::to_string(|s| s.print_variant(item)),
                path: ModPath::new(),
            };
            variants.push(variant_link);
        }
        let mut links = HashMap::new();
        links.insert(DocType::Variant, variants);
        links
    }
}

impl Convert<String> for ast::FnDecl {
    fn convert(&self, _context: &Context) -> String {
        pprust::to_string(|s| s.print_fn_args_and_ret(self))
    }
}

impl Convert<String> for ast::Expr {
    fn convert(&self, _context: &Context) -> String {
        pprust::expr_to_string(self)
    }
}

impl Convert<String> for ast::Ident {
    fn convert(&self, _context: &Context) -> String {
        pprust::ident_to_string(*self)
    }
}

impl Convert<String> for ast::Name {
    fn convert(&self, _context: &Context) -> String {
        pprust::to_string(|s| s.print_name(*self))
    }
}

impl Convert<String> for ast::Mac {
    fn convert(&self, _context: &Context) -> String {
        pprust::mac_to_string(self)
    }
}

impl Convert<Attributes> for [ast::Attribute] {
    fn convert(&self, _context: &Context) -> Attributes {
        Attributes::from_ast(self)
    }
}
