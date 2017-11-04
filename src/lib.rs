#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate error_chain;
#[macro_use]
extern crate log;
#[macro_use]
extern crate serde_derive;

extern crate clap;
extern crate ansi_term;
extern crate bincode;
extern crate cursive;
extern crate env_logger;
extern crate regex;
extern crate serde;
extern crate strsim;
extern crate syntex_syntax as syntax;
extern crate toml;
extern crate term_size;
extern crate catmark;

#[cfg(unix)]
extern crate pager;

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
pub mod ast_ty_wrappers;
pub mod errors;
