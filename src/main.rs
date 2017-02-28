extern crate toml;
extern crate syntex_syntax as syntax;
extern crate serde_json;

#[macro_use]
extern crate serde_derive;

#[macro_use]
extern crate error_chain;

mod driver;
mod paths;
mod store;

use store::Store;

use std::fmt;
use std::fmt::Display;
use std::collections::HashMap;
use std::env;
use std::path::{Path, PathBuf};
use std::io::{self, Read, Write};
use std::fs::{create_dir_all, File};

use syntax::ast;
use syntax::abi;
use syntax::print::pprust;
use syntax::codemap::Spanned;
use syntax::codemap::{CodeMap, Loc, Span};
use syntax::diagnostics::plugin::DiagnosticBuilder;
use syntax::parse::{self, ParseSess};
use syntax::visit::{self, FnKind, Visitor};

mod errors {
    // Create the Error, ErrorKind, ResultExt, and Result types
    error_chain! { }
}

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

#[derive(Debug, Serialize, Deserialize)]
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

#[derive(Debug, Serialize, Deserialize)]
pub struct ModPath(Vec<PathSegment>);

impl ModPath {
    fn from_ident(span: Span, ident: ast::Ident) -> ModPath {
        ModPath(
            ast::Path::from_ident(span, ident).segments.iter().map(
                |seg| PathSegment { identifier: pprust::ident_to_string(seg.identifier) }).collect()
        )
    }
    fn push(&mut self, seg: PathSegment) {
        self.0.push(seg);
    }
    fn pop(&mut self) {
        self.0.pop();
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
struct FnDoc {
    path: ModPath,
    signature: String,
    unsafety: Unsafety,
    constness: Constness,
    // TODO: Generics
    visibility: Visibility,
    abi: Abi,
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

fn fn_to_string(p: &ast::FnDecl) -> String {
    pprust::to_string(|s| s.print_fn_args_and_ret(p))
}

fn main() {
    if let Err(ref e) = run() {
        println!("error: {}", e);

        for e in e.iter().skip(1) {
            println!("caused by: {}", e);
        }

        // The backtrace is not always generated. Try to run this example
        // with `RUST_BACKTRACE=1`.
        if let Some(backtrace) = e.backtrace() {
            println!("backtrace: {:?}", backtrace);
        }

        ::std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        bail!("Wrong number of args (1 expected)");
    }

    let path = cache_doc_for_crate(&args[1])?;

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
    arg_counts: HashMap<String, usize>,
    // The codemap is necessary to go from a `Span` to actual line & column
    // numbers for closures.
    codemap: &'a CodeMap,
    store: Store,
}

impl<'a> RustdocCacher<'a> {
    fn format_span(&self, span: Span) -> String {
        format!("{}-{}",
                format_loc(&self.codemap.lookup_char_pos(span.lo)),
                format_loc(&self.codemap.lookup_char_pos(span.hi)))
    }
}

fn format_loc(loc: &Loc) -> String {
    format!("{}:{}", loc.line, loc.col.0)
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
        let fn_name = match fn_kind {
            FnKind::ItemFn(id, gen, unsafety, Spanned{ node: constness, span: span }, abi, visibility, block) => {
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

                let my_path =  ModPath::from_ident(span, id);
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
            ast::ItemKind::Mod(ref module) => {
                println!("Entering {:?}", item.ident);
                self.store.push_path(item.ident);
            },
            _ => (),
        }
        visit::walk_item(self, item);

        match item.node {
            ast::ItemKind::Mod(ref module) => {
                println!("Leaving {:?}", item.ident);
                self.store.pop_path()
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

fn generate_doc_cache(krate: &ast::Crate, codemap: &CodeMap, crate_info: CrateInfo) -> Result<()> {
    
    let mut visitor = RustdocCacher {
        arg_counts: HashMap::new(),
        codemap: codemap,
        store: Store::new(&crate_info).unwrap(),
    };

    visitor.visit_mod(&krate.module, krate.span, ast::CRATE_NODE_ID);

    Ok(())
}
#[test]
fn test_save_load_store() {
    let pathbuf = cache_doc_for_crate(&"/home/nuko/.cargo/registry/src/github.com-1ecc6299db9ec823/bytecount-0.1.6/".to_string()).unwrap();

    let mut store = Store::new(pathbuf, "bytecount".to_string());
    store.load_cache();
}
