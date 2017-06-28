#[macro_use] extern crate lazy_static;
#[macro_use] extern crate error_chain;
#[macro_use] extern crate log;
#[macro_use] extern crate serde_derive;
#[macro_use] extern crate clap;
extern crate ansi_term;
extern crate bincode;
extern crate cursive;
extern crate env_logger;
extern crate pager;
extern crate regex;
extern crate serde;
extern crate syntex_syntax as syntax;
extern crate toml;

pub mod convert;
pub mod document;
pub mod driver;
pub mod generator;
mod io_support;
pub mod markup;
pub mod paths;
pub mod store;
mod toml_util;
pub mod tui;
pub mod visitor;

pub mod errors {
    // Create the Error, ErrorKind, ResultExt, and Result types
    error_chain! { }
}

