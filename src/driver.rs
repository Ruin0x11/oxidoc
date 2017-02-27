use ::PathSegment;
use ::errors::*;
use std::io::{Read, Write};
use std::fs::File;


struct Driver {
    writer: Write,
}

fn parse_name(name: String) -> Vec<PathSegment> {
    name.split("::").map(|s| PathSegment { identifier: s.to_string() }).collect()
}

impl Driver {
    fn display_names(&self, names: Vec<String>) {
        for name in names {
            let name_exp = self.expand_name(name);
        }
    }

    fn expand_name(&self, name: String) -> String {
        let path_segs: Vec<PathSegment> = parse_name(name);

        // TODO: Detect if a type prefix is used (:fn)

        path_segs.iter().fold(String::new(), |res, s| res + &s.identifier)
    }

    fn display_name(&self, name: String) -> Result<()> {
        // TODO: Functions only.
        if let Err(_) = self.display_method(name) {
            bail!({"No method found"})
        }

        Ok(())
    }

    fn display_method(&self, name: String) -> Result<()> {
        // let mut out = Document::new();

        // out.add_method(name);

        // out.display();
        Ok(())
    }
}
