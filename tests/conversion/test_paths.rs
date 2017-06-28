use oxidoc::convert::NewDocTemp_;
use oxidoc::document::{CrateInfo, ModPath};
use oxidoc::generator;

use syntax::parse::{self, ParseSess};

fn print_paths(paths: &Vec<ModPath>) -> String {
    let mut s = String::new();
    for path in paths.iter() {
        s.push_str(&format!("{}\n", path));
    }
    s
}

fn make_docs(doc_str: &str) -> Vec<NewDocTemp_> {
    let parse_session = ParseSess::new();
    let krate = match parse::parse_crate_from_source_str("test.rs".to_string(), doc_str.to_string(), &parse_session) {
        Ok(_) if parse_session.span_diagnostic.has_errors() => panic!("Parse error"),
        Ok(krate) => krate,
        Err(_) => panic!("Failed to parse"),
    };

    let crate_info = CrateInfo {
        name: "crate".to_string(),
        version: "1.0.0".to_string(),
    };

    generator::generate_crate_docs(krate, crate_info).unwrap()
}

fn assert_paths_found(converted: &Vec<NewDocTemp_>, paths: Vec<&str>) {
    let expected_paths: Vec<ModPath> = paths.into_iter().map(|s| ModPath::from(s.to_string())).collect();
    let found_paths: Vec<ModPath> = converted.iter().map(|d| d.mod_path.clone()).collect();
    assert!(found_paths == expected_paths, "\nFound\n====\n{}\n\nExpected\n{}\n",
            print_paths(&found_paths),
            print_paths(&expected_paths));
}

#[test]
fn test_no_modules() {
    let doc = make_docs("");
    assert_paths_found(&doc, vec!["crate"]);
}


#[test]
fn test_one_module() {
    let doc = make_docs("mod test { }");
    assert_paths_found(&doc, vec!["crate",
                                  "crate::test"]);
}


#[test]
fn test_nested_modules() {
    let doc = make_docs(r#"
mod a {
    mod b { }
}"#);
    assert_paths_found(&doc, vec!["crate",
                                  "crate::a",
                                  "crate::a::b"]);
}
