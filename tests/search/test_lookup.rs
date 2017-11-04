use oxidoc::store::Store;
use oxidoc::generation;
use util;

fn add_docs(store: &mut Store, krate_name: &str, version: &str, src: &str) {
    let docs = util::source_to_docs(src);
    let docset = generation::make_docset(docs).unwrap();

    store.add_docset(util::get_crate_info(krate_name, version), docset);
}

fn store_from_source(src: &str) -> Store {
    store_from_crate_source("crate", "1.0.0", src)
}

fn store_from_crate_source(krate_name: &str, version: &str, src: &str) -> Store {
    let mut store = Store::new();
    add_docs(&mut store, krate_name, version, src);

    store
}

fn assert_search_query(store: &Store, query: &str, expected_paths: Vec<&str>) {
    let results = store.lookup_name(query);
    let mut found_paths: Vec<String> = results.into_iter().map(|r| r.mod_path.to_string()).collect();
    let mut expected_paths: Vec<String> = expected_paths.iter().map(|p| p.to_string()).collect();

    found_paths.sort();
    expected_paths.sort();

    assert!(found_paths == expected_paths, "\nSearch results for {}:\nFound\n========\n{}\n\nExpected\n========\n{}\n\n",
            query,
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
    assert_search_query(&store, "a", vec!["crate::a",
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

#[test]
fn test_search_for_removed_item() {
    let mut store = Store::new();
    add_docs(&mut store, "crate", "0.0.1", r#"
pub mod stuff {
    pub fn depreciated() {}
}
"#);
    add_docs(&mut store, "crate", "0.1.0", r#"
pub mod stuff {
}
"#);
    assert_search_query(&store, "stuff::depreciated", vec!["crate::stuff::depreciated"]);
}
