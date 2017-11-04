use oxidoc::conversion::Documentation;
use oxidoc::document::{CrateInfo, ModPath};
use oxidoc::generation;

use syntax::codemap::FilePathMapping;
use syntax::parse::{self, ParseSess};
use syntax::ast;

pub fn get_crate_info(name: &str, version: &str) -> CrateInfo {
    CrateInfo {
        name: name.to_string(),
        version: version.to_string(),
        lib_path: None,
    }
}

fn parse_crate_from_source(docs_string: String) -> ast::Crate {
    let parse_session = ParseSess::new(FilePathMapping::empty());

    let result = parse::parse_crate_from_source_str("test.rs".to_string(), docs_string,
                                                    &parse_session);

    match result {
        Ok(_) if parse_session.span_diagnostic.has_errors() => panic!("Parse error"),
        Ok(krate) => krate,
        Err(_) => panic!("Failed to parse"),
    }
}

pub fn source_to_docs(docs_str: &str) -> Vec<Documentation> {
    let krate = parse_crate_from_source(docs_str.to_string());

    let crate_info = get_crate_info("crate", "1.0.0");
    let l = generation::generate_crate_docs(krate, crate_info).unwrap();
    for i in l.iter() {
        debug!("{}", i.mod_path);
    }
    l
}

pub fn print_paths(paths: &Vec<ModPath>) -> String {
    let strings: Vec<String> = paths.iter().cloned().map(|p| p.to_string()).collect();
    strings.join("\n")
}
