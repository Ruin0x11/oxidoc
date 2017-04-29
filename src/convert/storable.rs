use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::Path;

use bincode::{self, Infinite};
use serde::{Serialize, Deserialize};

use convert::NewDocTemp_;

pub type StoreResult<T> = Result<T, ()>;

pub trait Storable: Serialize + Deserialize {
    fn save<T: AsRef<Path>>(&self, path_: &T) -> StoreResult<()>;
}

impl Storable for NewDocTemp_ {
    fn save<T: AsRef<Path>>(&self, path_: &T) -> StoreResult<()> {
        let path = path_.as_ref();

        let full_path = path.join(self.to_filepath());
        fs::create_dir_all(full_path.parent().unwrap()).map_err(|_| ())?;

        let mut doc_file = File::create(full_path).map_err(|_| ())?;
        let doc_data = bincode::serialize(self, Infinite).map_err(|_| ())?;

        doc_file.write(doc_data.as_slice()).map_err(|_| ())?;

        Ok(())
    }
}
