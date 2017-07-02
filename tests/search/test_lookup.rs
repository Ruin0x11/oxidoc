use oxidoc::store::Store;
use oxidoc::generator;
use util;

fn store_from_source(src: &str) -> Store {
    let mut store = Store::new();
    let docs = util::source_to_docs(src);
    let docset = generator::make_docset(docs).unwrap();

    store.add_docset(util::get_crate_info(), docset);

    store
}

fn assert_search_query(store: &Store, query: &str, expected_paths: Vec<&str>) {
    let results = store.lookup_name(query);
    let mut found_paths: Vec<String> = results.into_iter().map(|r| r.mod_path.to_string()).collect();
    let mut expected_paths: Vec<String> = expected_paths.iter().map(|p| p.to_string()).collect();

    found_paths.sort();
    expected_paths.sort();

    assert!(found_paths == expected_paths, "\nFound\n====\n{}\n\nExpected\n====\n{}\n",
               found_paths.join("\n"),
               expected_paths.join("\n"));
}

#[test]
fn test_search_ignores_case() {
    let store = store_from_source("pub struct Test;");
    assert_search_query(&store, "test", vec!["crate::Test"]);
}

#[test]
fn test_nested_search() {
    let store = store_from_source(r#"
pub mod a {
    pub mod b {
        pub fn test() {}
    }
}
"#);
    assert_search_query(&store, "crate", vec!["crate",
                                              "crate::a",
                                              "crate::a::b",
                                              "crate::a::b::test"]);
}

#[test]
fn test_same_segment_name() {
    let store = store_from_source(r#"
pub mod nyanko {
    pub struct Nyanko;
}
"#);
    assert_search_query(&store, "nyanko::Nyanko", vec!["crate::nyanko::Nyanko"]);
}
