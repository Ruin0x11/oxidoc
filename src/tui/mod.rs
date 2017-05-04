mod search;
mod sorted_result_set;
mod score;

extern crate cursive;

use std::sync::Mutex;

use cursive::Cursive;
use cursive::views::{EditView, LinearLayout, Dialog, SelectView, TextView};
use cursive::align::HAlign;
use cursive::traits::*;

use convert::NewDocTemp_;
use driver::Driver;
use markup::{MarkupDoc, Format};
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

    show_search_screen(&mut siv);

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

fn update_search_results(siv: &mut Cursive, query: &str) {
    let mut results = siv.find_id::<SelectView<usize>>("results").unwrap();
    results.clear();

    let matches = run_query(query);

    for (label, idx) in matches.into_iter() {
        results.add_item(label, idx);
    }
}

fn show_search_screen(siv: &mut Cursive) {
    let mut result_list: SelectView<usize> = SelectView::new().h_align(HAlign::Center);
    let mut search_box = EditView::new();

    // Sets the callback for when "Enter" is pressed.
    result_list.set_on_submit(show_next_window);

    // Not fast enough for realtime fuzzy matching...
    // search_box.set_on_edit(update_search_results);
    search_box.set_on_submit(update_search_results);

    let layout = LinearLayout::new(cursive::direction::Orientation::Vertical)
        .child(search_box.with_id("search_box"))
        .child(result_list.with_id("results"));

    // Let's add a BoxView to keep the list at a reasonable size - it can scroll anyway.
    siv.add_layer(Dialog::around(layout.fixed_size((80, 40)))
        .title("Documentation search"));

    update_search_results(siv, "");
}

// Let's put the callback in a separate function to keep it clean, but it's not required.
fn show_next_window(siv: &mut Cursive, idx: &usize) {
    let doc_markup = get_markup(idx);

    show_doc(siv, doc_markup)
}

fn get_markup(idx: &usize) -> MarkupDoc {
    let result: NewDocTemp_ = Driver::get_doc(PATHS.lock().unwrap().get(*idx).unwrap()).unwrap();
    result.format()
}

fn show_doc(siv: &mut Cursive, doc: MarkupDoc) {
    let text = format!("{}", doc);

    siv.add_layer(Dialog::around(TextView::new(text))
                  .button("Back", |s| s.pop_layer()));
}
