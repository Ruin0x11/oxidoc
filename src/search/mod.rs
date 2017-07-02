mod matcher;
mod score;
mod search;
mod sorted_result_set;

use std::sync::Mutex;

use convert::NewDocTemp_;
use driver::Driver;
use markup::{MarkupDoc, Format};
use store::{Store, StoreLocation};
use ::errors::*;
use self::search::Search;
use strsim::levenshtein;

lazy_static! {
    static ref PATHS: Mutex<Vec<StoreLocation>> = Mutex::new(Vec::new());
}

pub fn add_search_paths(paths: Vec<StoreLocation>) {
    PATHS.lock().unwrap().extend(paths);
}

pub fn run_query(query: &str) -> Vec<(String, usize)> {
    let lines: Vec<String> = PATHS.lock().unwrap().iter().map(|l| l.to_string()).collect();

    let search = Search::blank(&lines, None, 40).append_to_search(query);
    let mut results = Vec::new();
    for position in 0..search.visible_limit {
        match search.result.get(position) {
            Some(element) => results.push((element.original.clone(), element.idx)),
            None          => (),
        }
    }

    results.sort_by_key(|&(ref path, _)| levenshtein(&path, query));
    results
}

pub fn get_store_location(idx: usize) -> Option<StoreLocation> {
    PATHS.lock().unwrap().get(idx).cloned()
}
