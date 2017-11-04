use conversion::Documentation;
use store::{self, StoreLocation};
use errors::*;

mod errors {
    error_chain! {
        errors {
            NoDocumentationFound {
                description("No documentation could be found.")
            }
        }
    }
}

pub struct Driver {}

impl Driver {
    pub fn new() -> Driver {
        Driver { }
    }

    pub fn get_doc(location: &StoreLocation) -> Result<Documentation> {
        let path = location.to_filepath();
        store::deserialize_object(path)
    }
}
