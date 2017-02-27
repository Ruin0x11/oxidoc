use ::PathSegment;
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
        let path_segs = parse_name(name);

        // TODO: Detect if a type prefix is used (:fn)

        segs.map(|s| s.identifier ).join("::");
    }
}
