#[macro_use] extern crate log;
extern crate lazy_static;
extern crate error_chain;
extern crate clap;
extern crate ansi_term;
extern crate bincode;
extern crate cursive;
extern crate env_logger;
extern crate regex;
extern crate serde;
extern crate syntex_syntax as syntax;
extern crate toml;
extern crate oxidoc;

#[cfg(unix)]
extern crate pager;

mod conversion;
mod search;
mod util;
