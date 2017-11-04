use conversion::Documentation;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use bincode::{self, Infinite};
use serde::de::DeserializeOwned;
use serde::ser::Serialize;
use strsim::levenshtein;

use conversion::DocType;
use document::CrateInfo;
use document::ModPath;
use paths;
use std::env;

#[test]
fn test_read_write_bincode() {
    let string = "Test.".to_string();
    let mut dir = env::temp_dir();
    dir.push("test.txt");

    serialize_object(&string, &dir).expect("Write failed");
    let result: String = deserialize_object(&dir).expect("Read failed");

    assert_eq!(string, result);
}

#[test]
fn test_store_loc_to_path() {
    let loc = StoreLocation {
        name: "Test".to_string(),
        crate_info: CrateInfo {
            name: "test".to_string(),
            version: "0.1.0".to_string(),
            lib_path: None,
        },
        mod_path: ModPath::from("crate::thing".to_string()),
        doc_type: DocType::Struct,
    };

    let path = loc.to_filepath().display().to_string();
    assert!(path.contains("test-0.1.0"), "{}", path);
    assert!(path.contains("crate"), "{}", path);
    assert!(path.contains("thing"), "{}", path);
    assert!(path.contains("sdesc-Test.odoc"), "{}", path);
}

#[test]
fn test_compare_version_numbers() {
    let assert_second_newer = |a, b| assert!(version_number_hash(a) < version_number_hash(b),
                                             "{} {}", a, b);
    assert_second_newer("0.1.0", "0.2.0");
    assert_second_newer("0.1.0", "1.0.0");
    assert_second_newer("0.1.0", "1.0.1");
    assert_second_newer("0.0.1", "0.1.0");
}
