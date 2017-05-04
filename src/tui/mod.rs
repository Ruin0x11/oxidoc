mod search;
mod sorted_result_set;
mod score;

extern crate cursive;

use std::sync::Mutex;

use cursive::Cursive;
use cursive::views::{EditView, LinearLayout, Dialog, SelectView, TextView};
use cursive::align::HAlign;
use cursive::traits::*;

use driver::Driver;
use markup::Format;
use store::{Store, StoreLocation};
use self::search::Search;
use ::errors::*;

lazy_static! {
    static ref PATHS: Mutex<Vec<StoreLocation>> = Mutex::new(Vec::new());
}

pub fn run() {
    let store = Store::load();
    PATHS.lock().unwrap().extend(store.all_locations());

    let mut siv = Cursive::new();

    let mut result_list: SelectView<usize> = SelectView::new().h_align(HAlign::Center);
    let mut search_box = EditView::new();

    // Sets the callback for when "Enter" is pressed.
    result_list.set_on_submit(show_next_window);

    search_box.set_on_edit(update_search_results);

    let layout = LinearLayout::new(cursive::direction::Orientation::Vertical)
        .child(search_box.with_id("search_box"))
        .child(result_list.with_id("results"));

    // Let's add a BoxView to keep the list at a reasonable size - it can scroll anyway.
    siv.add_layer(Dialog::around(layout.fixed_size((80, 40)))
        .title("Documentation search"));

    update_search_results(&mut siv, "", 0);

    siv.add_global_callback('q', move |s| s.quit());

    siv.run();
}

fn run_query(query: &str) -> Vec<(String, usize)> {
    let lines: Vec<String> = PATHS.lock().unwrap().iter().map(|l| l.to_string()).collect();

    let search = Search::blank(&lines, None, 40).append_to_search(query);
    let mut results = Vec::new();
    for position in 0..search.visible_limit {
        match search.result.get(position) {
            Some(element) => results.push((element.original.clone(), element.idx)),
            None          => (),
        }
    }
    results
}

fn update_search_results(siv: &mut Cursive, query: &str, _len: usize) {
    let mut results = siv.find_id::<SelectView<usize>>("results").unwrap();
    results.clear();

    let matches = run_query(query);

    for (label, idx) in matches.into_iter() {
        results.add_item(label, idx);
    }
}

// Let's put the callback in a separate function to keep it clean, but it's not required.
fn show_next_window(siv: &mut Cursive, idx: &usize) {
    siv.pop_layer();

    show_doc(siv, PATHS.lock().unwrap().get(*idx).unwrap())
}


fn show_doc(siv: &mut Cursive, loc: &StoreLocation) {
    let result = Driver::get_doc(loc).unwrap();
    let doc = result.format();

    let text = format!("{} is a great loc!", doc);
    siv.add_layer(Dialog::around(TextView::new(text))
                  .button("Quit", |s| s.quit()));
}
