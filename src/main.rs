#[macro_use]
extern crate error_chain;
#[macro_use]
extern crate log;
#[macro_use]
extern crate clap;

extern crate ansi_term;
extern crate bincode;
extern crate cursive;
extern crate env_logger;
extern crate regex;
extern crate serde;
extern crate syntex_syntax as syntax;
extern crate toml;
extern crate catmark;

extern crate oxidoc;

use std::path::PathBuf;

use clap::{App, Arg};
use oxidoc::driver::Driver;
use oxidoc::generation;
use oxidoc::errors::*;
use oxidoc::store::StoreLocation;
use oxidoc::markup::Format;
use oxidoc::store::Store;

#[cfg(unix)]
extern crate pager;
#[cfg(unix)]
use pager::Pager;

fn app<'a, 'b>() -> App<'a, 'b> {
    App::new(format!("oxidoc {}", crate_version!()))
        .about("A command line interface to Rustdoc.")
        .arg(Arg::with_name("version").short("V").long("version").help(
            "Prints version info",
        ))
        .arg(Arg::with_name("tui").short("t").long("tui").help(
            "Starts interactive console user interface",
        ))
        .arg(
            Arg::with_name("generate")
                .short("g")
                .long("generate")
                .value_name("CRATE_DIR")
                .help(
                    "Generate oxidoc info for the specified crate root directory, 'std' for stdlib \
                    (requires RUST_SRC_PATH to be set), 'crates' for all cargo crates or 'all' \
                    for everything",
                )
                .takes_value(true)
                .alias("generate"),
        )
        .arg(Arg::with_name("pager").short("p").long("pager").help(
            "Automatically pages output",
        ))
        .arg(Arg::with_name("query").index(1))
}

fn main() {
    env_logger::init().unwrap();

    if let Err(ref e) = run() {
        error!("error: {}", e);

        for e in e.iter().skip(1) {
            error!("caused by: {}", e);
        }

        if let Some(backtrace) = e.backtrace() {
            error!("backtrace: {:?}", backtrace);
        }

        ::std::process::exit(1);
    }
}

fn generate(arg: Option<&str>) -> Result<()> {
    match arg {
        Some("all") => generation::generate_all_docs(),
        Some("crates") => generation::generate_crate_registry_docs(),
        Some("std") => generation::generate_stdlib_docs(),
        Some(x) => generation::generate_docs_for_path(PathBuf::from(x)),
        None => bail!(ErrorKind::NoCrateDirectoryProvided),
    }
}

fn run() -> Result<()> {
    let matches = app().get_matches();
    if matches.is_present("version") {
        println!("oxidoc {}", crate_version!());
        return Ok(());
    }

    if matches.is_present("generate") {
        return generate(matches.value_of("generate"));
    }

    if matches.is_present("tui") {
        oxidoc::tui::run()
    } else {
        let query = match matches.value_of("query") {
            Some(x) => x,
            None => bail!(ErrorKind::NoSearchQuery),
        };

        let enable_pager = matches.is_present("pager");
        print_search_query(query, enable_pager)
    }
}

#[cfg(windows)]
fn setup_pager() {}

#[cfg(unix)]
fn setup_pager() {
    let executable = get_pager_executable();

    Pager::with_pager(&executable).setup();
}

#[cfg(unix)]
fn get_pager_executable() -> String {
    if let Ok(pager) = std::env::var("PAGER") {
        return pager.to_string();
    }

    // Linux and BSD systems doesn't support "-r" option but macOS supports
    // For linux `less` shows better results (with control chars)
    #[cfg(target_os = "macos")]
    let executable = "more -r";

    #[cfg(not(target_os = "macos"))]
    let executable = "less";

    return executable.to_string();
}

fn print_search_query(query: &str, enable_pager: bool) -> Result<()> {
    let store = Store::load();
    // search::add_search_paths(store.all_locations());

    let results: Vec<&StoreLocation> = store.lookup_name(query).into_iter().take(10).collect();

    if results.is_empty() {
        println!("No results for \"{}\".", query);
        return Ok(());
    }

    let formatted: Vec<String> = results
        .into_iter()
        .map(|location| {
            let result = Driver::get_doc(&location).unwrap();

            result.format().to_string()
        })
        .collect();

    if enable_pager {
        setup_pager();
    }

    for result in formatted {
        println!("{}", result);
    }

    Ok(())
}
