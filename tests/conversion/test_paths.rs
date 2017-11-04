use oxidoc::conversion::Documentation;
use oxidoc::document::ModPath;

use util::{source_to_docs, print_paths};

fn assert_paths_found(converted: &Vec<Documentation>, mut paths: Vec<&str>) {
    let mut converted_strings: Vec<String> = converted
        .iter()
        .map(|doc| doc.mod_path.to_string())
        .collect();

    converted_strings.sort();
    paths.sort();

    let expected_paths: Vec<ModPath> = paths
        .into_iter()
        .map(|s| ModPath::from(s.to_string()))
        .collect();
    let found_paths: Vec<ModPath> = converted_strings
        .into_iter()
        .map(|d| ModPath::from(d))
        .collect();

    assert!(
        found_paths == expected_paths,
        "\nExpected\n====\n{}\n\nFound\n====\n{}\n",
        print_paths(&found_paths),
        print_paths(&expected_paths)
    );
}

#[test]
fn test_no_modules() {
    let docs = source_to_docs("");
    assert_paths_found(&docs, vec!["crate"]);
}

#[test]
fn test_one_module() {
    let docs = source_to_docs("pub mod test { }");
    assert_paths_found(&docs, vec!["crate", "crate::test"]);
}

#[test]
fn test_one_struct() {
    let docs = source_to_docs("pub mod test { pub struct MyStruct; }");
    assert_paths_found(&docs, vec!["crate", "crate::test", "crate::test::MyStruct"]);
}

#[test]
fn test_doc_hidden() {
    let docs = source_to_docs(
        r#"
pub mod test {
    #[doc(hidden)]
    pub struct MyStruct;

    impl MyStruct {
        pub fn method() {}
    }
}"#,
    );
    assert_paths_found(&docs, vec!["crate", "crate::test"]);
}

#[test]
fn test_one_method() {
    let docs = source_to_docs(
        r#"
pub mod test {
    pub struct MyStruct;

    impl MyStruct {
        pub fn method() {}
    }
}"#,
    );
    assert_paths_found(
        &docs,
        vec![
            "crate",
            "crate::test",
            "crate::test::MyStruct",
            "crate::test::MyStruct::method",
        ],
    );
}

#[test]
fn test_nested_modules() {
    let docs = source_to_docs(
        r#"
pub mod a {
    pub mod b { }
}"#,
    );
    assert_paths_found(&docs, vec!["crate", "crate::a", "crate::a::b"]);
}

#[cfg(never)]
#[test]
fn test_private_module() {
    let docs = source_to_docs("mod a { }");
    assert_paths_found(&docs, vec!["crate"]);
}

#[cfg(never)]
#[test]
fn test_use_super() {
    let docs = source_to_docs(
        r#"
pub struct MyStruct;

pub mod a {
    use super::MyStruct;

    impl MyStruct {
        pub fn test_a(&self) {}
    }
}"#,
    );
    assert_paths_found(
        &docs,
        vec![
            "crate",
            "crate::MyStruct",
            "crate::MyStruct::test_a",
            "crate::a",
        ],
    )
}

#[cfg(never)]
#[test]
fn test_later_use() {
    let docs = source_to_docs(
        r#"
pub mod b {
    pub mod a {
      pub struct MyStruct;
    }
    pub use self::a::MyStruct;
}
impl b::MyStruct {
    pub fn method_a() {}
}
use b::MyStruct;
impl MyStruct {
    pub fn method_b() {}
}
"#,
    );
    assert_paths_found(
        &docs,
        vec![
            "crate",
            "crate::a",
            "crate::b",
            "crate::a::MyStruct",
            "crate::a::MyStruct::method_a",
            "crate::a::MyStruct::method_b",
        ],
    );
}

#[cfg(never)]
#[test]
fn test_use_globbed() {
    let docs = source_to_docs(
        r#"
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
"#,
    );
    assert_paths_found(
        &docs,
        vec![
            "crate",
            "crate::a",
            "crate::a::MyStruct",
            "crate::a::MyStruct::test_a",
            "crate::b",
        ],
    )
}

#[test]
fn test_separate_impl() {
    let docs = source_to_docs(r#"
pub struct Test;

impl Test {
    pub fn foo() {}
}

impl Test {
    pub fn bar() {}
}
"#);
    assert_paths_found(
        &docs,
        vec![
            "crate",
            "crate::Test",
            "crate::Test::foo",
            "crate::Test::bar"
        ]
    );
}

#[test]
fn test_separate_nested_impl() {
    let docs = source_to_docs(r#"
pub struct Test;

impl Test {
    pub fn foo() {}
}

pub mod a {
    use ::Test;
    impl Test {
        pub fn bar() {}
    }
}
"#);
    assert_paths_found(
        &docs,
        vec![
            "crate",
            "crate::Test",
            "crate::Test::foo",
            "crate::Test::bar",
            "crate::Test::a"
        ]
    );
}
