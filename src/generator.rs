use std;
use store::Store;
use toml;

use std::fmt;
use std::fmt::Display;
use std::env;
use std::path::{Path, PathBuf};
use std::io::{Read};
use std::fs::{File};

use syntax::ast;
use syntax::abi;
use syntax::print::pprust;
use syntax::codemap::Spanned;
use syntax::codemap::{CodeMap, Span};
use syntax::diagnostics::plugin::DiagnosticBuilder;
use syntax::parse::{self, ParseSess};
use syntax::visit::{self, FnKind, Visitor};

use errors::*;

// Since we can't derive Serialize/Deserialize on ast's types.
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

#[derive(Clone, Debug, Serialize, Deserialize)]
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

#[derive(Clone, Debug, Serialize, Deserialize)]
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

    pub fn parent(&self) -> ModPath {
        let mut n = self.clone();
        n.0.pop();
        ModPath(n.0)
    }

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
#[derive(Debug, Serialize, Deserialize)]
struct Package {
    name: String,
    version: String,
}

/// Holds the TOML fields we care about when deserializing
#[derive(Debug, Serialize, Deserialize)]
pub struct CrateInfo {
    package: Package,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FnDoc {
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
        let str = self.path.to_string();
        write!(f, "{}", str)
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

pub fn generate(src_dir: String) -> Result<()> {
    cache_doc_for_crate(&src_dir).
        chain_err(|| format!("Unable to generate documentation for directory {}", src_dir))?;

    Ok(())
}

/// Generates cached Rustdoc information for the given crate.
/// Expects the crate root directory as an argument.
fn cache_doc_for_crate(crate_path_name: &String) -> Result<()> {

    let crate_path = Path::new(crate_path_name.as_str());
    let toml_path = crate_path.join("Cargo.toml");

    let mut fp = File::open(&toml_path).chain_err(|| format!("Could not find Cargo.toml in path {}", toml_path.display()))?;

    let ref mut contents = String::new();
    fp.read_to_string(contents).chain_err(|| "Failed to read from file")?;

    let info: CrateInfo = toml::de::from_str(contents).chain_err(|| "Couldn't parse Cargo.toml")?;

    let parse_session = ParseSess::new();

    // TODO: This has to handle [lib] targets and multiple [[bin]] targets.
    let mut main_path = crate_path.join("src/lib.rs");
    if !main_path.exists() {
        main_path = crate_path.join("src/main.rs");
        if!main_path.exists() {
            bail!("No crate entry point found (nonstandard paths are unsupported)");
        }
    }
    let krate = parse(main_path.as_path(), &parse_session).unwrap();

    generate_doc_cache(&krate, parse_session.codemap(), info)
        .chain_err(|| "Failed to generate doc cache")
}

struct RustdocCacher<'a> {
    // The codemap is necessary to go from a `Span` to actual line & column
    // numbers for closures.
    codemap: &'a CodeMap,
    store: Store,
    current_scope: ModPath,
}

impl<'a> RustdocCacher<'a> {
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

impl<'v, 'a> Visitor<'v> for RustdocCacher<'a> {
    //fn visit_fn(&mut self,
    //fk: FnKind<'ast>, fd: &'ast FnDecl, s: Span, _: NodeId) {
    fn visit_fn(&mut self,
                fn_kind: FnKind<'v>,
                fn_decl: &'v ast::FnDecl,
                //block: &'v ast::Block,
                span: Span,
                _id: ast::NodeId) {
         match fn_kind {
            FnKind::ItemFn(id, gen, unsafety, Spanned{ node: constness, span: span }, abi, visibility, _) => {
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

                let my_path = ModPath::join(&ModPath::from_ident(span, id),
                                            &self.current_scope);
                let doc = FnDoc {
                    path: my_path,
                    signature: sig,
                    unsafety: my_unsafety,
                    constness: my_constness,
                    // TODO: Generics
                    visibility: my_visibility,
                    abi: my_abi,
                };

                self.store.write_fn(doc);
            },
            FnKind::Method(id, _, _, _) => {
                //TODO: This makes sense only in the context of an impl / Trait
                //id.name.as_str().to_string(),
            },
            FnKind::Closure(_) => () // Don't care.
        };


        // Continue walking the rest of the funciton so we pick up any functions
        // or closures defined in its body.
        visit::walk_fn(self, fn_kind, fn_decl, span);
    }

    // The default implementation panics, so this is needed to work on files
    // with macro invocations, eg calls to `format!()` above. A better solution
    // would be to expand macros before walking the AST, but I haven't looked at
    // how to do that. We will miss any functions defined via a macro, but
    // that's fine for this example.
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

    // fn visit_mod(&mut self, m: &'v ast::Mod, span: Span, node: ast::NodeId) {
    //     for item in &m.items {
    //         //let my_path = ModPath::from_ident(span, item.ident);
    //         // NOTE: Use the ItemKind of an Item to determine if it's doc'ed.
    //         match item.node {
    //             ast::ItemKind::Mod(..) => println!("Mod: {}", item.ident),
    //             _                      => (),
    //         }
    //     }
    //     visit::walk_mod(self, m);
    // }
}

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

fn generate_doc_cache(krate: &ast::Crate, codemap: &CodeMap, crate_info: CrateInfo) -> Result<()> {
    
        let crate_doc_path = get_crate_doc_path(&crate_info)
            .chain_err(|| format!("Unable to get crate doc path for crate: {}", crate_info.package.name))?;
    let mut visitor = RustdocCacher {
        codemap: codemap,
        store: Store::new(crate_doc_path).unwrap(),
        current_scope: ModPath(Vec::new()),
    };

    visitor.visit_mod(&krate.module, krate.span, ast::CRATE_NODE_ID);

    visitor.store.save_cache()
        .chain_err(|| "Couldn't save cache for module")
}
