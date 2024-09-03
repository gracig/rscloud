use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::io;
use std::sync::Arc;
use thiserror::Error;

use super::plan::ResourceItem;
use super::registry::{ResourceType, SharedRegistry};
use super::storage::file::FileStorage;

#[derive(Clone)]
pub struct Datastore {
    inner: HashMap<String, Vec<u8>>,
    storage: Arc<dyn Storage + Send + Sync>,
}
impl Default for Datastore {
    fn default() -> Self {
        Datastore::new(FileStorage::default())
    }
}
pub trait Storage {
    fn load(&self) -> Result<HashMap<String, Vec<u8>>, DatastoreError>;
    fn save(&self, data: &HashMap<String, Vec<u8>>) -> Result<(), DatastoreError>;
}

#[derive(Serialize, Deserialize)]
struct ResourceItemWrapper {
    resource_type: ResourceType,
    resource: Value,
}

impl Datastore {
    pub fn new(storage: impl Storage + 'static + Send + Sync) -> Self {
        Self {
            inner: Default::default(),
            storage: Arc::new(storage),
        }
    }
    pub fn reload(&mut self) -> Result<HashMap<String, Vec<u8>>, DatastoreError> {
        println!("--- Load datastore resources from storage ---");
        self.storage
            .load()
            .map_err(|e| DatastoreError::LoadError(e.to_string()))
            .inspect(|h| {
                for k in h.keys() {
                    println!("Resource[{}] loaded from datastore", k);
                }
            })
            .map(|data| std::mem::replace(&mut self.inner, data))
    }
    pub fn save(&self) -> Result<(), DatastoreError> {
        println!("--- Save datastore resources to storage ---");
        self.storage.save(&self.inner)
    }
    fn insert_bytes(&mut self, id: impl Into<String>, value: Vec<u8>) -> Option<Vec<u8>> {
        let id = id.into();
        println!("Insert Resource[{}] to datastore", id);
        self.inner.insert(id, value)
    }
    pub fn insert_resource(
        &mut self,
        registry: &SharedRegistry,
        resource: &dyn ResourceItem,
    ) -> Result<Option<Vec<u8>>, DatastoreError> {
        let id = resource.id();
        let resource_type = resource.resource_type();
        let resource = ResourceItemWrapper {
            resource: registry.serialize_resource(resource, &resource_type)?,
            resource_type,
        };
        serde_json::to_value(resource)
            .and_then(|value| serde_json::to_vec(&value))
            .map_err(DatastoreError::JsonError)
            .map(|bytes| {
                // Convert the Option<Vec<u8>> to fit Result<Option<Vec<u8>>, DatastoreError>
                self.insert_bytes(id, bytes)
            })
    }

    pub fn get(
        &mut self,
        registry: &SharedRegistry,
        id: &str,
    ) -> Result<Option<Arc<dyn ResourceItem>>, DatastoreError> {
        self.get_bytes(id).map_or(Ok(None), |data| {
            serde_json::from_slice::<ResourceItemWrapper>(data)
                .map_err(DatastoreError::JsonError)
                .and_then(|wrapper| {
                    registry
                        .deserialize_resource(&wrapper.resource, &wrapper.resource_type)
                        .map(Some)
                        .map_err(DatastoreError::RegistryError)
                })
        })
    }
    fn get_bytes(&mut self, id: &str) -> Option<&Vec<u8>> {
        self.inner.get(id)
    }
    pub fn remove(&mut self, id: &str) -> Option<Vec<u8>> {
        println!("Remove Resource[{}] from datastore", id);
        self.inner.remove(id)
    }
    pub fn contains(&self, id: &str) -> bool {
        self.inner.contains_key(id)
    }
    pub fn keys(&self) -> Vec<String> {
        self.inner.keys().cloned().collect()
    }
}

#[derive(Debug, Error)]
pub enum DatastoreError {
    #[error("PoisonError")]
    PoisonError,
    #[error("Load Error error: {0}")]
    LoadError(String),
    #[error("IO Error error: {0}")]
    IOError(#[from] io::Error),
    #[error("Serialization or deserialization error: {0}")]
    BincodeError(#[from] bincode::Error),
    #[error("Serialization or deserialization error: {0}")]
    JsonError(#[from] serde_json::Error),
    #[error("Serialization or deserialization error")]
    InsertionError,
    #[error("Serialization or deserialization error")]
    RegistryError(#[from] super::registry::RegistryError),
}
