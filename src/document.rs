use std::fmt::{self, Display};
use std::path::PathBuf;
use std::slice;

use syntax::ast;
use syntax::codemap::{Span};
use syntax::print::pprust;

/// Represents a single portion of a full module path.
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

/// Represents a module path, like `std::fmt`. Used for easily resolving crate modules to their
/// on-disk documentation locations.
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

    pub fn tail(&self) -> ModPath {
        let (_, tail) = self.0.split_at(1);
        ModPath(tail.clone().to_vec())
    }

    pub fn join(first: &ModPath, other: &ModPath) -> ModPath {
        let mut result = first.clone();
        result.0.extend(other.0.iter().cloned());
        result
    }

    pub fn to_filepath(&self) -> PathBuf {
        PathBuf::from(self.0.iter().fold(String::new(), |res, s| res + &s.identifier.clone() + "/"))
    }

    pub fn segments(&self) -> slice::Iter<PathSegment> {
        self.0.iter()
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

/// Holds the name and version of a crate to generate its documentation directory.
#[derive(Clone, Debug, Serialize, Deserialize, Hash, Eq, PartialEq)]
pub struct CrateInfo {
    pub name: String,
    pub version: String,
    pub lib_path: Option<String>,
}

impl CrateInfo {
    pub fn to_path_prefix(&self) -> PathBuf {
        PathBuf::from(format!("{}-{}", self.name, self.version))
    }
}

impl Display for CrateInfo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}-{}", self.name, self.version)
    }
}
