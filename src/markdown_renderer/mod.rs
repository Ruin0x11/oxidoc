mod ansi_renderer;
mod dombox;

use pulldown_cmark::Parser;
use pulldown_cmark::{Options, OPTION_ENABLE_TABLES, OPTION_ENABLE_FOOTNOTES};

pub fn render_ansi(text: &str, width: u16) -> String {
    let mut opts = Options::empty();
    opts.insert(OPTION_ENABLE_TABLES);
    opts.insert(OPTION_ENABLE_FOOTNOTES);
    let p = Parser::new_ext(&text, opts);
    ansi_renderer::push_ansi(p, width)
}
