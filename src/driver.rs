use serde::ser::{Serialize};
use serde::de::{Deserialize};
use paths;
use store::*;
use std::collections::HashMap;
use std::fmt;
use std::fmt::Display;
use document::{ModPath};
use convert::*;
use convert::NewDocTemp_;

mod errors {
    error_chain! {
        errors {
            NoDocumentationFound {
                description("No documentation could be found.")
            }
        }
    }

}
use errors::*;

fn expand_name(name: &String) -> Result<ModPath> {
    let segs = ModPath::from(name.clone());
    Ok(segs)
}

pub struct Driver {

}

impl Driver {
    pub fn new() -> Driver {
        Driver { }
    }
}

#[cfg(test)]
mod test {
    use super::*;
}
