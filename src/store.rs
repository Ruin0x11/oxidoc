use convert::Documentation;
use std::collections::{HashMap, HashSet};
use std::env;
use std::fmt;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use bincode::{self, Infinite};
use serde::de::DeserializeOwned;
use serde::ser::Serialize;
use strsim::levenshtein;

use convert::DocType;
use document::CrateInfo;
use document::ModPath;
use ::errors::*;

const STORE_FILENAME: &str = "store";

pub fn get_doc_registry_path() -> Result<PathBuf> {
    let home_dir = if let Some(dir) = env::home_dir() {
        dir
    } else {
        bail!("Could not locate home directory");
    };

    Ok(home_dir.as_path().join(".cargo/registry/doc"))
}

/// Obtains the base output path for a crate's documentation.
pub fn get_crate_doc_path(crate_info: &CrateInfo) -> Result<PathBuf> {
    let registry_path = get_doc_registry_path()?;

    let path = registry_path.join(format!("{}-{}",
                                          crate_info.name,
                                          crate_info.version));
    Ok(path)
}


fn get_store_file() -> Result<PathBuf> {
    let mut registry_path = get_doc_registry_path()?;
    registry_path.push(STORE_FILENAME);
    Ok(registry_path)
}

fn create_or_open_file<T: AsRef<Path>>(path: T) -> Result<File> {
    let path_as = path.as_ref();
    if !path_as.exists() {
        OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(path_as)
            .chain_err(|| format!("Could not create file {}", path_as.display()))
    } else {
        OpenOptions::new()
            .read(true)
            .write(true)
            .open(path_as)
            .chain_err(|| format!("Could not open file {}", path_as.display()))
    }
}

pub fn deserialize_object<S, T>(path: T) -> Result<S>
    where S: DeserializeOwned,
          T: AsRef<Path>
{
    let path_as = path.as_ref();
    let mut data: Vec<u8> = Vec::new();
    let mut bincoded_file = File::open(&path_as)
        .chain_err(|| format!("Could not open file {}", path_as.display()))?;

    bincoded_file.read_to_end(&mut data)
        .chain_err(|| format!("Failed to read file {}", path_as.display()))?;
    let result = bincode::deserialize(data.as_slice())
        .chain_err(|| format!("Could not deserialize file at {}", path_as.display()))?;

    Ok(result)
}

pub fn serialize_object<S, T>(data: &S, path: T) -> Result<()>
    where S: Serialize,
          T: AsRef<Path>
{
    let path_as = path.as_ref();

    let data = bincode::serialize(data, Infinite)
        .chain_err(|| format!("Could not serialize data for {}", path_as.display()))?;

    let mut bincoded_file = create_or_open_file(path_as)?;
    bincoded_file.write(data.as_slice())
        .chain_err(|| format!("Failed to write file {}", path_as.display()))?;

    Ok(())
}

type CrateVersion = String;
type CrateName = String;
type CrateVersions = HashMap<CrateVersion, Docset>;
type DocumentCorpus = HashMap<CrateName, CrateVersions>;
type ModuleExpansions = HashMap<String, HashSet<String>>;

#[derive(Serialize, Deserialize)]
pub struct Store {
    /// "serde" => "1.0.0" => Docset { /* ... */}
    items: DocumentCorpus,

    /// A map from individual module path segments to fully resolved module paths that use them.
    /// "vec" => ["std::vec::Vec", ...]
    module_expansions: ModuleExpansions,
}

impl Store {
    pub fn new() -> Self {
        Store {
            items: HashMap::new(),
            module_expansions: HashMap::new(),
        }
    }

    pub fn load() -> Self {
        match Store::load_from_disk() {
            Ok(store) => store,
            Err(_)    => Store::new(),
        }
    }

    pub fn save(&mut self) -> Result<()> {
        let store_file = get_store_file()?;
        serialize_object(self, store_file)
    }

    pub fn load_from_disk() -> Result<Self> {
        let store_file = get_store_file()?;
        deserialize_object(store_file)
    }

    pub fn add_docset(&mut self, crate_info: CrateInfo, docset: Docset) {
        // TODO: Any way to remove old module expansions if docset is regenerated?
        for doc in docset.documents.values() {
            for segment in doc.mod_path.0.iter() {
                let mod_path = doc.mod_path.to_string().to_lowercase();

                let mut entry = self.module_expansions
                    .entry(segment.identifier.to_lowercase())
                    .or_insert(HashSet::new());

                entry.insert(mod_path);
            }
        }

        let mut entry = self.items.entry(crate_info.name).or_insert(HashMap::new());
        entry.insert(crate_info.version, docset);
    }

    pub fn all_locations(&self) -> Vec<StoreLocation> {
        let mut results = Vec::new();
        for krate in self.items.values() {
            for version in krate.values() {
                results.extend(version.documents.values().cloned().collect::<Vec<StoreLocation>>());
            }
        }
        results
    }

    pub fn lookup_name(&self, query: &str) -> Vec<&StoreLocation> {
        let mut results = Vec::new();

        let matches = get_all_matching_paths(query.to_string(), &self.module_expansions);

        for mat in matches {
            let krate_name = mat.split("::").next().unwrap().to_string();

            // TODO: select based on latest version
            let res: Option<&StoreLocation> =
                if let Some(krate_versions) = self.items.get(&krate_name) {
                    if let Some(version) = latest_version(krate_versions) {
                        krate_versions.get(version).and_then(|docset| {
                            let path = ModPath::from(mat.clone()).tail().to_string();
                            docset.documents.get(&path)
                        })
                    } else {
                        None
                    }
                } else {
                    None
                };

            if let Some(loc) = res {
                results.push(loc);
            }
        }

        results.sort_by_key(|loc| levenshtein(query, &loc.mod_path.to_string()));

        results
    }
}

fn latest_version(versions: &CrateVersions) -> Option<&CrateVersion> {
    let mut max = None;
    let mut res = None;
    for version in versions.keys() {
        let hash = version_number_hash(version);
        if max.map_or(true, |m| hash > m) {
            res = Some(version);
            max = Some(hash);
        }
    }
    res
}

/// Returns the module paths which contain all the provided path segments
fn get_all_matching_paths(query: String,
                          module_expansions: &ModuleExpansions)
                          -> Vec<String> {
    let query_lower = query.to_lowercase();
    let path_segments: Vec<String> = query_lower.split("::").map(|s| s.to_string()).collect();

    let mut result = Vec::new();

    for segment in path_segments.into_iter() {
        if let Some(matches) = module_expansions.get(&segment) {
            if result.is_empty() {
                result = matches.iter().cloned().collect();
            } else {
                result = intersect(result, &matches)
            }
        }
    }

    result.retain(|res| res.to_lowercase().contains(&query_lower));

    result
}

fn intersect(target: Vec<String>, other: &HashSet<String>) -> Vec<String> {
    let mut common = Vec::new();
    let mut v_other: Vec<_> = other.iter().collect();

    for e1 in target.into_iter() {
        if let Some(pos) = v_other.iter().position(|e2| e1 == **e2) {
            common.push(e1);
            v_other.remove(pos);
        }
    }

    common
}

fn version_number_hash(version: &str) -> u64 {
    let slice: Vec<String> = version.split(".").map(|s| s.to_string()).collect();
    if slice.len() != 3 {
        return 0;
    }
    let a = slice[0].parse::<u64>().unwrap();
    let b = slice[1].parse::<u64>().unwrap();
    let c = slice[2].parse::<u64>().unwrap();
    (a << 16) + (b << 8) + c
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Docset {
    /// Map from relative module path to location
    /// "vec::Vec" => StoreLocation { name: Vec, /* ... */ }
    pub documents: HashMap<String, StoreLocation>,
}

impl Docset {
    pub fn new() -> Self {
        Docset {
            documents: HashMap::new(),
        }
    }

    pub fn add_docs(&mut self, documents: Vec<Documentation>) -> Result<()> {
        for doc in documents.into_iter() {
            let relative_path = doc.mod_path.tail().to_string();
            self.documents.insert(relative_path.to_lowercase(), doc.to_store_location());
            doc.save()
                .chain_err(|| format!("Could not add doc {} to docset", doc.mod_path))?;
        }
        Ok(())
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct StoreLocation {
    pub name: String,
    pub crate_info: CrateInfo,
    pub mod_path: ModPath,
    pub doc_type: DocType,
}

impl StoreLocation {
    pub fn new(name: String,
               crate_info: CrateInfo,
               mod_path: ModPath,
               doc_type: DocType) -> Self
    {
        StoreLocation {
            name: name,
            crate_info: crate_info,
            mod_path: mod_path,
            doc_type: doc_type,
        }
    }

    pub fn to_filepath(&self) -> PathBuf {
        let mut path = get_crate_doc_path(&self.crate_info).unwrap();
        let doc_path = self.mod_path.to_filepath();
        path.push(doc_path);
        let filename = format!("{}{}.odoc", self.doc_type.get_file_prefix(), self.name);
        path.push(filename);
        path
    }
}

impl fmt::Display for StoreLocation {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} ({} {})", self.mod_path, self.crate_info.name, self.crate_info.version)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_write_bincode() {
        let string = "Test.".to_string();
        let path = PathBuf::from("/tmp/test.txt");

        serialize_object(&string, &path).expect("Write failed");
        let result: String = deserialize_object(&path).expect("Read failed");

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
        assert!(path.contains("test-0.1.0/crate/thing/sdesc-Test.odoc"), "{}", path);
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
}
