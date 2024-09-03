use std::collections::HashMap;
use std::fs::File;
use std::path::PathBuf;

use crate::v1::datastore::{DatastoreError, Storage};

#[derive(Clone)]
pub struct FileStorage {
    path: PathBuf,
}

impl Default for FileStorage {
    fn default() -> Self {
        Self::new("fpcloud.store")
    }
}
impl FileStorage {
    pub fn new<P: Into<PathBuf>>(path: P) -> Self {
        FileStorage { path: path.into() }
    }
}

impl Storage for FileStorage {
    fn load(&self) -> Result<HashMap<String, Vec<u8>>, DatastoreError> {
        Ok(if self.path.exists() {
            let file = File::open(&self.path)?;
            bincode::deserialize_from(file)?
        } else {
            HashMap::new()
        })
    }
    fn save(&self, data: &HashMap<String, Vec<u8>>) -> Result<(), DatastoreError> {
        let file = File::create(&self.path)?;
        bincode::serialize_into(file, data)?;
        Ok(())
    }
}
