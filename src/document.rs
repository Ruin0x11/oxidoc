use std::fmt::{self, Display};
use std::path::{Path, PathBuf};
use syntax::ast;
use syntax::codemap::Spanned;
use syntax::codemap::{Span};
use syntax::print::pprust;

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

#[derive(Debug, Serialize, Deserialize)]
pub enum Constness {
    Const,
    NotConst,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Visibility {
    Public,
    Private
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
    pub fn parent(&self) -> ModPath {
        let mut n = self.clone();
        n.0.pop();
        ModPath(n.0)
    }

    /// The final segment of the path.
    pub fn name(&self) -> PathSegment {
        let seg = self.0.iter().last();
        seg.unwrap().clone()
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

pub trait Document {
    fn render(&self, f: &mut fmt::Formatter) -> fmt::Result;
}

impl Display for Document {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.render(f)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StructDoc {
    pub crate_info: CrateInfo,
    pub path: ModPath,
    pub signature: String,

    pub fn_docs: Vec<DocSig>,
}

impl Document for StructDoc {
    fn render(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // TODO: docstrings are currently not built into the AST.
        write!(f, r#"
(from crate {})
=== {}()
------------------------------------------------------------------------------
  {}

------------------------------------------------------------------------------

{}
"#, self.crate_info, self.path, self.signature, "<Docstring will go here.>")
    }
}

impl Display for StructDoc {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.render(f)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FnDoc {
    pub crate_info: CrateInfo,
    pub path: ModPath,
    pub signature: String,

    pub unsafety: Unsafety,
    pub constness: Constness,
    // TODO: Generics
    pub visibility: Visibility,
    pub abi: Abi,
}

impl Document for FnDoc {
    fn render(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // TODO: docstrings are currently not built into the AST.
        write!(f, r#"
(from crate {})
=== {}()
------------------------------------------------------------------------------
  {}

------------------------------------------------------------------------------

{}
"#, self.crate_info, self.path, self.signature, "<Docstring will go here.>")
    }
}

impl Display for FnDoc {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.render(f)
    }
}
