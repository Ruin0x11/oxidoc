#[macro_use]
extern crate clap;
extern crate toml;
extern crate syntex_syntax as syntax;
extern crate serde_json;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate error_chain;

mod generator;
mod driver;
mod paths;
mod store;

use driver::Driver;

use clap::{App, Arg};

mod errors {
    // Create the Error, ErrorKind, ResultExt, and Result types
    error_chain! { }
}

use errors::*;

fn app<'a, 'b>() -> App<'a, 'b> {
    App::new(format!("rd {}", crate_version!()))
        .about("The command line interface to Rustdoc.")
        .arg(Arg::with_name("version")
             .short("V")
             .long("version")
             .help("Print version info"))
        .arg(Arg::with_name("generate")
             .short("g")
             .long("generate")
             .value_name("GENERATE")
             .help("Generate rd info for the specified crate")
             .takes_value(true)
             .alias("generate"))
        .arg(Arg::with_name("query")
             .index(1))
}

fn main() {
    if let Err(ref e) = run() {
        println!("error: {}", e);

        for e in e.iter().skip(1) {
            println!("caused by: {}", e);
        }

        // The backtrace is not always generated. Try to run this example
        // with `RUST_BACKTRACE=1`.
        if let Some(backtrace) = e.backtrace() {
            println!("backtrace: {:?}", backtrace);
        }

        ::std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let matches = app().get_matches();
    if matches.is_present("version") {
        println!("rd {}", crate_version!());
        return Ok(())
    }

    if matches.is_present("generate") {
        match matches.value_of("generate") {
            Some(x) => {
                return generator::generate(x.to_string())
            },
            None => bail!("Failed to generate rd info.")
        }
    }

    let query = match matches.value_of("query") {
        Some(x) => x,
        None => bail!("No search query was provided.")
    };

    let driver = Driver::new();
    let mut v = Vec::new();
    v.push(query.to_string());
    driver.display_names(v)
        .chain_err(|| "don't work")
}
