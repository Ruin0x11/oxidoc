use serde::ser::{Serialize};
use serde::de::{Deserialize};
use std::fs::{File, create_dir_all};
use std::io::{Read, Write};
use serde_json;
use std::fmt::{self, Display};
use std::path::{Path, PathBuf};
use syntax::ast;
use syntax::abi;
use syntax::codemap::Spanned;
use syntax::codemap::{Span};
use syntax::print::pprust;
use syntax::visit;
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
#[derive(Debug, Serialize, Deserialize)]
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

#[derive(Debug, Serialize, Deserialize)]
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

#[derive(Debug, Serialize, Deserialize)]
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

#[derive(Debug, Serialize, Deserialize)]
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

#[derive(Debug, Serialize, Deserialize)]
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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Document<T: Documentable> {
    pub doc: T,
    pub crate_info: CrateInfo,
    pub path: ModPath,
    pub signature: String,
    pub docstring: String,
}

impl<T: Documentable + Serialize + Deserialize> Document<T> {
    fn render(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, r#"
(from crate {})
=== {}
------------------------------------------------------------------------------
  {}

------------------------------------------------------------------------------

{}
"#, self.crate_info, self.doc.get_info(&self.path), self.signature, self.docstring)
    }

    pub fn get_filename(&self) -> Result<String> {
        let docfile = paths::encode_doc_filename(&self.path.name().unwrap().identifier)
            .chain_err(|| "Could not encode doc filename")?;

        info!("Encoded name as {}", docfile);

        Ok(T::get_filename(docfile))
    }

    fn get_docfile(&self, store_path: &PathBuf) -> Result<PathBuf> {
        println!("Attempting to write docfile under {} for {}", store_path.display(), self.path);
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

    /// Attempt to load the documentation at 'scope' from the store.
    pub fn load_doc(path: PathBuf, full_path: &ModPath) -> Result<Self> {
        let identifier = full_path.name().unwrap().identifier.clone();
        let encoded_name = paths::encode_doc_filename(&identifier)
            .chain_err(|| "Could not encode doc filename")?;

        let scope = full_path.parent().unwrap();

        let doc_path = path.join(scope.to_path())
            .join(T::get_filename(encoded_name));

        info!("Attempting to load doc at {}", &doc_path.display());
        let mut fp = File::open(&doc_path)
            .chain_err(|| format!("Couldn't find oxidoc store {}", doc_path.display()))?;

        let mut json = String::new();
        fp.read_to_string(&mut json)
            .chain_err(|| format!("Couldn't read oxidoc store {}", doc_path.display()))?;

        info!("Loading {}", doc_path.display());
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

/// All documentation information for a struct.
#[derive(Debug, Serialize, Deserialize)]
pub struct StructDoc_ {
    pub fn_docs: Vec<DocSig>,
}

impl Documentable for StructDoc_ {
    fn get_info(&self, path: &ModPath) -> String {
        path.to_string()
    }
    fn get_filename(name: String) -> String {
        format!("sdesc-{}.odoc", name)
    }
}

pub type StructDoc = Document<StructDoc_>;

/// All documentation information for a function.
#[derive(Debug, Serialize, Deserialize)]
pub struct FnDoc_ {
    pub unsafety: Unsafety,
    pub constness: Constness,
    // TODO: Generics
    pub visibility: Visibility,
    pub abi: Abi,
    pub ty: FnKind,
}

impl Documentable for FnDoc_ {
    fn get_info(&self, path: &ModPath) -> String {
        match self.ty {
            FnKind::ItemFn => format!("{}()", path),
            FnKind::Method => format!("(impl on {})", path.parent().unwrap()),
            FnKind::MethodFromImpl => format!("(impl on {})", path.parent().unwrap()),
            FnKind::MethodFromTrait => format!("<from trait>"),
        }
    }
    fn get_filename(name: String) -> String {
        format!("{}.odoc", name)
    }
}

pub type FnDoc = Document<FnDoc_>;

/// All documentation inormation for a module.
#[derive(Debug, Serialize, Deserialize)]
pub struct ModuleDoc_ {
    pub fn_docs: Vec<DocSig>,
    pub struct_docs: Vec<DocSig>,
    pub module_docs: Vec<DocSig>,
}

impl Documentable for ModuleDoc_ {
    fn get_info(&self, path: &ModPath) -> String {
        path.to_string()
    }
    fn get_filename(name: String) -> String {
        format!("mdesc-{}.odoc", name)
    }
}

pub type ModuleDoc = Document<ModuleDoc_>;
