use oxidoc::convert::NewDocTemp_;
use oxidoc::document::{CrateInfo, ModPath};
use oxidoc::generator;

use syntax::parse::{self, ParseSess};
use syntax::ast;

fn print_paths(paths: &Vec<ModPath>) -> String {
    let mut s = String::new();
    for path in paths.iter() {
        s.push_str(&format!("{}\n", path));
    }
    s
}

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

fn make_docs(docs_str: &str) -> Vec<NewDocTemp_> {
    let krate = parse_crate(docs_str.to_string());

    let crate_info = CrateInfo {
        name: "crate".to_string(),
        version: "1.0.0".to_string(),
    };

    generator::generate_crate_docs(krate, crate_info).unwrap()
}

fn assert_paths_found(converted: &Vec<NewDocTemp_>, mut paths: Vec<&str>) {
    let mut converted_strings: Vec<String> = converted.iter()
        .map(|doc| doc.mod_path.to_string())
        .collect();

    converted_strings.sort();
    paths.sort();

    let expected_paths: Vec<ModPath> = paths.into_iter().map(|s| ModPath::from(s.to_string())).collect();
    let found_paths: Vec<ModPath> = converted_strings.into_iter().map(|d| ModPath::from(d)).collect();

    assert!(found_paths == expected_paths, "\nFound\n====\n{}\n\nExpected\n====\n{}\n",
            print_paths(&found_paths),
            print_paths(&expected_paths));
}

#[test]
fn test_no_modules() {
    let docs = make_docs("");
    assert_paths_found(&docs, vec!["crate"]);
}

#[test]
fn test_one_module() {
    let docs = make_docs("pub mod test { }");
    assert_paths_found(&docs, vec!["crate",
                                   "crate::test"]);
}

#[test]
fn test_one_struct() {
    let docs = make_docs("pub mod test { pub struct MyStruct; }");
    assert_paths_found(&docs, vec!["crate",
                                   "crate::test",
                                   "crate::test::MyStruct"]);
}

#[test]
fn test_one_method() {
    let docs = make_docs(r#"
pub mod test {
    pub struct MyStruct;

    impl MyStruct {
        pub fn method() {}
    }
}"#);
    assert_paths_found(&docs, vec!["crate",
                                   "crate::test",
                                   "crate::test::MyStruct",
                                   "crate::test::MyStruct::method"]);
}

#[test]
fn test_nested_modules() {
    let docs = make_docs(r#"
pub mod a {
    pub mod b { }
}"#);
    assert_paths_found(&docs, vec!["crate",
                                   "crate::a",
                                   "crate::a::b"]);
}

#[test]
fn test_private_module() {
    let docs = make_docs("mod a { }");
    assert_paths_found(&docs, vec!["crate"]);
}

#[test]
fn test_use_super() {
    let docs = make_docs(r#"
pub struct MyStruct;

pub mod a {
    use super::MyStruct;

    impl MyStruct {
        pub fn test_a(&self) {}
    }
}"#);
    assert_paths_found(&docs, vec!["crate",
                                   "crate::MyStruct",
                                   "crate::MyStruct::test_a",
                                   "crate::a"])
}

#[test]
fn test_later_use() {
    let docs = make_docs(r#"
mod b {
    mod a {
      pub struct MyStruct;
    }
    pub use self::a::MyStruct;
}
impl b::MyStruct {
    fn method_a() {}
}
use b::MyStruct;
impl MyStruct {
    fn method_b() {}
}
"#);
    assert_paths_found(&docs, vec!["crate",
                                   "crate::a",
                                   "crate::b",
                                   "crate::a::MyStruct",
                                   "crate::a::MyStruct::method_a",
                                   "crate::a::MyStruct::method_b"]);
}

#[test]
fn test_use_globbed() {
    let docs = make_docs(r#"
pub mod a {
    pub struct MyStruct;
}

pub mod b {
    use a::*;

    impl MyStruct {
        pub fn test_a(&self) {
        }
    }
}
"#);
    assert_paths_found(&docs, vec!["crate",
                                   "crate::a",
                                   "crate::a::MyStruct",
                                   "crate::a::MyStruct::test_a",
                                   "crate::b"])
}
