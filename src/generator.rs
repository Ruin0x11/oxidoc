use std;
use store::Store;
use toml;

use std::fmt;
use std::fmt::Display;
use std::env;
use std::path::{Path, PathBuf};
use std::io::{Read};
use std::fs::{File, remove_dir_all};

use syntax::ast;
use syntax::abi;
use syntax::print::pprust;
use syntax::codemap::Spanned;
use syntax::codemap::{Span};
use syntax::diagnostics::plugin::DiagnosticBuilder;
use syntax::parse::{self, ParseSess};
use syntax::visit::{self, FnKind, Visitor};

use paths;

use errors::*;

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
struct Package {
    name: String,
    version: String,
}

/// Holds the TOML fields we care about when deserializing
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CrateInfo {
    package: Package,
}

impl Display for CrateInfo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}-{}", self.package.name, self.package.version)
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

impl Display for FnDoc {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // TODO: docstrings are currently not built into the AST.
        let s = format!(r#"
(from crate {})
=== {}()
------------------------------------------------------------------------------
  {}

------------------------------------------------------------------------------

Description will go here.
"#, self.crate_info, self.path, self.signature);
        write!(f, "{}", s)
    }
}

fn parse<'a, T: ?Sized + AsRef<Path>>(path: &T,
                                      parse_session: &'a ParseSess)
                                      -> std::result::Result<ast::Crate, Option<DiagnosticBuilder<'a>>> {
    let path = path.as_ref();

    match parse::parse_crate_from_file(path, parse_session) {
        // There may be parse errors that the parser recovered from, which we
        // want to treat as an error.
        Ok(_) if parse_session.span_diagnostic.has_errors() => Err(None),
        Ok(krate) => Ok(krate),
        Err(e) => Err(Some(e)),
    }
}

pub fn generate_all() -> Result<()> {
    println!("Regenerating all documentation.");

    let home_dir: PathBuf;
    if let Some(x) = env::home_dir() {
        home_dir = x
    } else {
        bail!("Could not locate home directory");
    }

    let path = home_dir.as_path().join(".cargo/registry/doc");

    remove_dir_all(path)
        .chain_err(|| "Could not remove cargo doc directory")?;

    for src_dir in paths::src_iter(true, true)
        .chain_err(|| "Could not iterate cargo registry src directories")?
    {
        cache_doc_for_crate(&src_dir).
            chain_err(|| format!("Unable to generate documentation for directory {}", &src_dir.display()))?;
    }
    Ok(())
}


pub fn generate(src_dir: PathBuf) -> Result<()> {
    cache_doc_for_crate(&src_dir).
        chain_err(|| format!("Unable to generate documentation for directory {}", &src_dir.display()))?;

    Ok(())
}

/// Generates cached Rustdoc information for the given crate.
/// Expects the crate root directory as an argument.
fn cache_doc_for_crate(crate_path: &PathBuf) -> Result<()> {
    let toml_path = crate_path.join("Cargo.toml");

    let mut fp = File::open(&toml_path).chain_err(|| format!("Could not find Cargo.toml in path {}", toml_path.display()))?;

    let ref mut contents = String::new();
    fp.read_to_string(contents).chain_err(|| "Failed to read from file")?;

    let info: CrateInfo = toml::de::from_str(contents).chain_err(|| "Couldn't parse Cargo.toml")?;

    println!("Generating documentation for {}", &info);

    let parse_session = ParseSess::new();

    // TODO: This has to handle [lib] targets and multiple [[bin]] targets.
    let mut main_path = crate_path.join("src/lib.rs");
    if !main_path.exists() {
        main_path = crate_path.join("src/main.rs");
        if!main_path.exists() {
            // TODO: Look for [lib] / [[bin]] targets here
            println!("No crate entry point found (nonstandard paths are unsupported)");
            return Ok(())
        }
    }
    let krate = parse(main_path.as_path(), &parse_session).unwrap();

    let store = generate_doc_cache(&krate, info)
        .chain_err(|| "Failed to generate doc cache")?;

    // TODO: save all to disk once, not as we go
    store.save()
        .chain_err(|| "Couldn't save rd data for module")
}

struct RustdocCacher {
    store: Store,
    current_scope: ModPath,
    crate_info: CrateInfo,
}

impl RustdocCacher {
    /// Pushes a segment onto the current path scope.
    pub fn push_path(&mut self, ident: ast::Ident) {
        let seg = PathSegment{ identifier: pprust::ident_to_string(ident) };
        self.current_scope.push(seg);
    }

    /// Pops a segment from the current path scope.
    pub fn pop_path(&mut self) {
        self.current_scope.pop();
    }

}

impl<'v> Visitor<'v> for RustdocCacher {
    //fn visit_fn(&mut self,
    //fk: FnKind<'ast>, fd: &'ast FnDecl, s: Span, _: NodeId) {
    fn visit_fn(&mut self,
                fn_kind: FnKind<'v>,
                fn_decl: &'v ast::FnDecl,
                //block: &'v ast::Block,
                span: Span,
                _id: ast::NodeId) {
        match fn_kind {
            FnKind::ItemFn(id, gen, unsafety, Spanned{ node: constness, span: _ }, abi, visibility, _) => {
                let sig = pprust::to_string(|s| s.print_fn(fn_decl, unsafety, constness,
                                                           abi, Some(id), gen, visibility));

                // convert ast types to our Serializable types.

                let my_unsafety = match unsafety {
                    ast::Unsafety::Normal => Unsafety::Normal,
                    ast::Unsafety::Unsafe => Unsafety::Unsafe,
                };

                let my_constness = match constness {
                    ast::Constness::Const    => Constness::Const,
                    ast::Constness::NotConst => Constness::NotConst,
                };

                let my_visibility = match *visibility {
                    ast::Visibility::Public => Visibility::Public,
                    _                       => Visibility::Private,
                };

                let my_abi = match abi {
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
                };

                let my_path = ModPath::join(&self.current_scope,
                                            &ModPath::from_ident(span, id));
                let doc = FnDoc {
                    crate_info: self.crate_info.clone(),
                    path: my_path,
                    signature: sig,
                    unsafety: my_unsafety,
                    constness: my_constness,
                    // TODO: Generics
                    visibility: my_visibility,
                    abi: my_abi,
                };

                self.store.add_fn(doc);
            },
            FnKind::Method(_, _, _, _) => {
                //TODO: This makes sense only in the context of an impl / Trait
                //id.name.as_str().to_string(),
            },
            FnKind::Closure(_) => () // Don't care.
        };


        // Continue walking the rest of the funciton so we pick up any functions
        // or closures defined in its body.
        visit::walk_fn(self, fn_kind, fn_decl, span);
    }

    fn visit_mac(&mut self, _mac: &'v ast::Mac) {
        // TODO: No, it isn't fine...
    }

    fn visit_item(&mut self, item: &'v ast::Item) {
        // Keep track of the path we're in as we traverse modules.
        match item.node {
            ast::ItemKind::Mod(_) => {
                self.push_path(item.ident);
            },
            _ => (),
        }

        visit::walk_item(self, item);

        match item.node {
            ast::ItemKind::Mod(_) => {
                self.pop_path()
            },
            _ => (),
        }
    }
}

/// Obtains the base output path for a crate's documentation.
fn get_crate_doc_path(crate_info: &CrateInfo) -> Result<PathBuf> {
    let home_dir: PathBuf;
    if let Some(x) = env::home_dir() {
        home_dir = x
    } else {
        bail!("Could not locate home directory");
    }

    let path = home_dir.as_path().join(".cargo/registry/doc")
        .join(format!("{}-{}", crate_info.package.name, crate_info.package.version));
    Ok(path)
}

/// Generates documentation for the given crate.
fn generate_doc_cache(krate: &ast::Crate, crate_info: CrateInfo) -> Result<Store> {

    let crate_doc_path = get_crate_doc_path(&crate_info)
        .chain_err(|| format!("Unable to get crate doc path for crate: {}", &crate_info.package.name))?;

    // Clear out old doc path
    if crate_doc_path.exists() {
        remove_dir_all(&crate_doc_path)
            .chain_err(|| format!("Could not remove crate doc directory {}", &crate_doc_path.display()))?;
    }

    let mut visitor = RustdocCacher {
        store: Store::new(crate_doc_path).unwrap(),
        current_scope: ModPath(Vec::new()),
        crate_info: crate_info.clone(),
    };

    // Push the crate name onto the current namespace so
    // the module "module" will resolve to "crate::module"
    visitor.current_scope.push(PathSegment{
        identifier: crate_info.package.name.clone()
    });

    visitor.visit_mod(&krate.module, krate.span, ast::CRATE_NODE_ID);

    Ok(visitor.store)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_harness(s: &str) -> Result<Store> {
        let parse_session = ParseSess::new();
        let krate = match parse::parse_crate_from_source_str("test.rs".to_string(), s.to_string(), &parse_session) {
            Ok(_) if parse_session.span_diagnostic.has_errors() => bail!("Parse error"),
            Ok(krate) => krate,
            Err(_) => bail!("Failed to parse"),
        };

        let crate_info = CrateInfo {
            package: Package {
                name: "test".to_string(),
                version: "1.0.0".to_string(),
            }
        };

        generate_doc_cache(&krate, crate_info)
    }


    #[test]
    fn test_has_modules() {
        let store = test_harness(r#"
        mod a {
            mod b {
            }
        }"#).unwrap();
        let modules = store.get_modules();
        let p = &ModPath::from("test".to_string());
        assert!(modules.contains(p), true);
        let p = &ModPath::from("test::a".to_string());
        assert!(modules.contains(p), true);
        let p = &ModPath::from("test::a::b".to_string());
        assert!(modules.contains(p), true);
    }

    #[test]
    fn test_module_contains_fns() {
        let store = test_harness(r#"
        mod a {
            mod b {
                fn thing() {
                    println!("Hello, world!");
                }
            }
        }"#).unwrap();
        panic!("not ready")
    }
}
