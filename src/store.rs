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
use ::errors::*;

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

/// Mapping of version strings for a crate to the documentation for that crate version.
type CrateVersions = HashMap<CrateVersion, Docset>;

/// Top-level storage of all crates and their documents, organized by version.
type DocumentCorpus = HashMap<CrateName, CrateVersions>;

/// Mapping from module path keywords to the full module paths that use those keywords. Used for
/// quick lookup of documentation based on keywords.
type ModuleExpansions = HashMap<String, HashSet<String>>;

/// The central point for retrieving documentation. Stores a map of crate names to their versions,
/// which map to their individual documentation stores. Also contains a keyword prefix map for
/// quick documentation searching.
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
        let store_file = paths::store_file_path()?;
        serialize_object(self, store_file)
    }

    pub fn load_from_disk() -> Result<Self> {
        let store_file = paths::store_file_path()?;
        deserialize_object(store_file)
    }

    /// Add documentation for a specific version of a crate.
    pub fn add_docset(&mut self, crate_info: CrateInfo, docset: Docset) {
        // TODO: Any way to remove old module expansions if docset is regenerated?
        for doc in docset.documents.values() {
            self.add_module_expansions(doc);
        }

        let mut entry = self.items.entry(crate_info.name).or_insert(HashMap::new());
        entry.insert(crate_info.version, docset);
    }

    /// Adds the keywords for module paths in the provided document to the prefix map used for
    /// document loookup.
    fn add_module_expansions(&mut self, doc: &StoreLocation) {
        for segment in doc.mod_path.0.iter() {
            let mod_path = doc.mod_path.to_string().to_lowercase();

            let entry = self.module_expansions
                .entry(segment.identifier.to_lowercase())
                .or_insert(HashSet::new());

            entry.insert(mod_path);
        }
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

    /// Search the documentation store for a keyword and return the documents with a match inside
    /// their module paths.
    pub fn lookup_name(&self, query: &str) -> Vec<&StoreLocation> {
        let mut results = Vec::new();

        let matches = get_all_matching_paths(query.to_string(), &self.module_expansions);

        for mat in matches {
            if let Some(loc) = self.retrieve_match(mat) {
                results.push(loc);
            }
        }

        results.sort_by_key(|loc| levenshtein(query, &loc.mod_path.to_string()));

        results
    }

    /// Searches the documentation store for the given fully resolved module path string.
    fn retrieve_match(&self, mat: String) -> Option<&StoreLocation> {
        let krate_name = mat.split("::").next().unwrap().to_string();

        let path_in_krate = ModPath::from(mat.clone());
        self.latest_doc_with_match(&krate_name, path_in_krate)
    }

    /// Retrieves the latest documentation for a crate matching the given module path
    fn latest_doc_with_match(&self, krate_name: &str, path_in_krate: ModPath) -> Option<&StoreLocation> {
        // FIXME: Doesn't handle items that exist in old versions and removed in the latest version
        if let Some(krate_versions) = self.items.get(krate_name) {
            if let Some(version) = latest_version(krate_versions) {
                krate_versions.get(version).and_then(|docset| {
                    let path = path_in_krate.tail().to_string();
                    docset.documents.get(&path)
                })
            } else {
                None
            }
        } else {
            None
        }
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

/// Returns the module paths which contain all the provided path segments.
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

/// Returns the strings that exist in both `target` and `other`.
fn intersect(target: Vec<String>, other: &HashSet<String>) -> Vec<String> {
    let mut in_common = Vec::new();
    let mut other_vec: Vec<_> = other.iter().collect();

    for e1 in target.into_iter() {
        if let Some(pos) = other_vec.iter().position(|e2| e1 == **e2) {
            in_common.push(e1);
            other_vec.remove(pos);
        }
    }

    in_common
}

/// Returns an integer that can be used to compare Semantic Versioning strings.
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

/// A set of documentation for a specific crate version.
#[derive(Serialize, Deserialize, Debug)]
pub struct Docset {
    /// Mapping from a crate-local module path string to the corresponding location
    /// "vec::Vec" => StoreLocation { name: Vec, /* ... */ }
    pub documents: HashMap<String, StoreLocation>,
}

impl Docset {
    pub fn new() -> Self {
        Docset {
            documents: HashMap::new(),
        }
    }

    fn add_doc(&mut self, document: Documentation) -> Result<()> {
        let relative_path = document.mod_path.tail().to_string();
        let store_location = document.to_store_location();
        self.documents.insert(relative_path.to_lowercase(), store_location);
        document.save()
            .chain_err(|| format!("Could not add doc {} to docset", document.mod_path))
    }

    pub fn add_docs(&mut self, documents: Vec<Documentation>) -> Result<()> {
        for doc in documents.into_iter() {
            self.add_doc(doc)?;
        }
        Ok(())
    }
}

/// Represents the on-disk location of a piece of documentation, with additional metadata on the
/// containing crate and type of object being documented.
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
        let mut path = paths::crate_doc_path(&self.crate_info).unwrap();
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
