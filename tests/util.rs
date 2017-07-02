use oxidoc::convert::NewDocTemp_;
use oxidoc::document::{CrateInfo, ModPath};
use oxidoc::generator;

use syntax::parse::{self, ParseSess};
use syntax::ast;

fn parse_crate(docs_string: String) -> ast::Crate {
    let parse_session = ParseSess::new();
    let result = parse::parse_crate_from_source_str("test.rs".to_string(), docs_string,
                                                    &parse_session);
    match result {
        Ok(_) if parse_session.span_diagnostic.has_errors() => panic!("Parse error"),
        Ok(krate) => krate,
        Err(_) => panic!("Failed to parse"),
    }
}

pub fn get_crate_info() -> CrateInfo {
    CrateInfo {
        name: "crate".to_string(),
        version: "1.0.0".to_string(),
        lib_path: None,
    }
}

pub fn source_to_docs(docs_str: &str) -> Vec<NewDocTemp_> {
    let krate = parse_crate(docs_str.to_string());

    let crate_info = get_crate_info();
    let l = generator::generate_crate_docs(krate, crate_info).unwrap();
    for i in l.iter() {
        println!("{}", i.mod_path);
    }
    l
}

pub fn print_paths(paths: &Vec<ModPath>) -> String {
    let strings: Vec<String> = paths.iter().cloned().map(|p| p.to_string()).collect();
    strings.join("\n")
}
