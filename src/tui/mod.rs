use std::sync::Mutex;

use convert::Documentation;
use cursive::{self, Cursive};
use cursive::align::HAlign;
use cursive::traits::*;
use cursive::views::{EditView, LinearLayout, Dialog, SelectView, TextView};
use driver::Driver;
use markup::{MarkupDoc, Format};
use store::{Store, StoreLocation};
use errors::*;

lazy_static! {
    static ref STORE: Mutex<Store> = Mutex::new(Store::load());
}

pub fn run() -> Result<()> {
    let mut siv = Cursive::new();

    show_search_screen(&mut siv);

    siv.add_global_callback('q', move |s| s.quit());

    siv.run();

    Ok(())
}

fn update_search_results(siv: &mut Cursive, query: &str, _length: usize) {
    let mut results = siv.find_id::<SelectView<StoreLocation>>("results").unwrap();
    results.clear();

    let matches: Vec<StoreLocation> = STORE
        .lock()
        .unwrap()
        .lookup_name(query)
        .into_iter()
        .cloned()
        .collect();

    for location in matches {
        results.add_item(location.mod_path.to_string(), location);
    }
}

fn show_search_screen(siv: &mut Cursive) {
    let mut result_list: SelectView<StoreLocation> = SelectView::new().h_align(HAlign::Center);
    let mut search_box = EditView::new();

    // Sets the callback for when "Enter" is pressed.
    result_list.set_on_submit(show_next_window);

    // Not fast enough for realtime fuzzy matching...
    search_box.set_on_edit(update_search_results);
    // search_box.set_on_submit(update_search_results);

    let layout = LinearLayout::new(cursive::direction::Orientation::Vertical)
        .child(search_box.with_id("search_box"))
        .child(result_list.with_id("results"));

    // Let's add a BoxView to keep the list at a reasonable size - it can scroll anyway.
    siv.add_layer(Dialog::around(layout.fixed_size((80, 40))).title(
        "Documentation search",
    ));

    update_search_results(siv, "", 0);
}

fn show_next_window(siv: &mut Cursive, location: &StoreLocation) {
    let doc_markup = get_markup(location);

    show_doc(siv, doc_markup)
}

fn get_markup(location: &StoreLocation) -> MarkupDoc {
    let result: Documentation = Driver::get_doc(location).unwrap();
    result.format()
}

fn show_doc(siv: &mut Cursive, doc: MarkupDoc) {
    let text = format!("{}", doc);

    siv.add_layer(Dialog::around(TextView::new(text)).button(
        "Back",
        |s| s.pop_layer(),
    ));
}
