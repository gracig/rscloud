use std::{
    collections::HashMap,
    fmt,
    sync::{Arc, Mutex},
};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use super::{
    manager::ResourceManager,
    plan::ResourceItem,
    resource::{Resource, SharedResource},
};

#[derive(Debug, Default, Clone, Eq, PartialEq, PartialOrd, Ord, Serialize, Deserialize, Hash)]
pub struct RProvider {
    pub name: String,
    pub region: String,
}

#[derive(Debug, Default, Clone, Eq, PartialEq, PartialOrd, Ord, Serialize, Deserialize, Hash)]
pub struct RType {
    pub name: String,
}

#[derive(Debug, Default, Clone, Eq, PartialEq, PartialOrd, Ord, Serialize, Deserialize, Hash)]
pub struct ResourceType {
    pub rprovider: RProvider,
    pub rtype: RType,
}

type Serializer = Arc<dyn Fn(&dyn ResourceItem) -> Result<Value, RegistryError> + Sync + Send>;
type Deserializer =
    Arc<dyn Fn(&Value) -> Result<Arc<dyn ResourceItem>, RegistryError> + Sync + Send>;

pub struct ResourceSerde {
    serializer: Serializer,
    deserializer: Deserializer,
}

pub trait ResourceSerdeProvider {
    fn get_resource_serde(
        &self,
        registry: &Registry,
        rt: &ResourceType,
    ) -> Result<ResourceSerde, RegistryError>;
}

#[derive(Clone)]
pub struct SharedRegistry {
    inner: Arc<Mutex<Registry>>,
}

unsafe impl Send for Registry {}

impl SharedRegistry {
    pub fn new(provider_factory: ProviderFactory) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Registry::new(provider_factory))),
        }
    }
    pub fn registry_provider(
        &self,
        rprodiver: &RProvider,
        provider: &Arc<Mutex<dyn ResourceSerdeProvider>>,
    ) {
        self.inner
            .lock()
            .unwrap()
            .register_provider(rprodiver, provider)
    }
    pub fn serialize_resource(
        &self,
        resource: &dyn ResourceItem,
        resource_type: &ResourceType,
    ) -> Result<Value, RegistryError> {
        self.inner
            .lock()
            .unwrap()
            .serialize_resource(resource, resource_type)
    }
    pub fn deserialize_resource(
        &self,
        value: &Value,
        resource_type: &ResourceType,
    ) -> Result<Arc<dyn ResourceItem>, RegistryError> {
        self.inner
            .lock()
            .unwrap()
            .deserialize_resource(value, resource_type)
    }
}

pub type ProviderFactory =
    Box<dyn Fn(&RProvider) -> Result<Arc<Mutex<dyn ResourceSerdeProvider>>, RegistryError>>;

pub struct Registry {
    serde_store: HashMap<ResourceType, ResourceSerde>,
    provider_store: HashMap<RProvider, Arc<Mutex<dyn ResourceSerdeProvider>>>,
    provider_fatory: ProviderFactory,
}

impl Registry {
    pub fn new(provider_factory: ProviderFactory) -> Self {
        Self {
            serde_store: HashMap::new(),
            provider_store: HashMap::new(),
            provider_fatory: provider_factory,
        }
    }
    pub fn register_provider(
        &mut self,
        rprovider: &RProvider,
        provider: &Arc<Mutex<dyn ResourceSerdeProvider>>,
    ) {
        self.provider_store
            .insert(rprovider.clone(), provider.clone());
    }

    pub fn serde<Input, Output>(
        &self,
        mngr: Arc<dyn ResourceManager<Input, Output>>,
    ) -> Result<ResourceSerde, RegistryError>
    where
        Input: 'static + Serialize + for<'de> Deserialize<'de> + Clone + fmt::Debug,
        Output: 'static + Serialize + for<'de> Deserialize<'de> + Clone + fmt::Debug,
    {
        let serialize_fn: Serializer = Arc::new(
            move |res: &dyn ResourceItem| -> Result<Value, RegistryError> {
                //println!("\nINSPECT_RESOURCE: {:?}", res);
                let res: Resource<Input, Output> = res
                    .as_any()
                    .downcast_ref::<SharedResource<Input, Output>>()
                    .ok_or(RegistryError::DowncastError)?
                    .resource
                    .lock()
                    .map_err(|_| RegistryError::LockResourceError)?
                    .clone();
                serde_json::to_value(res).map_err(RegistryError::SerializationError)
            },
        );
        let deserialize_fn: Deserializer = Arc::new({
            let resource_manager_clone: Arc<dyn ResourceManager<Input, Output>> = Arc::clone(&mngr);
            move |value: &Value| -> Result<Arc<dyn ResourceItem>, RegistryError> {
                serde_json::from_value(value.clone())
                    .map(|mut resource: Resource<Input, Output>| {
                        resource.manager = Some(resource_manager_clone.clone());
                        Arc::new(SharedResource::new(resource)) as Arc<dyn ResourceItem>
                    })
                    .map_err(RegistryError::SerializationError)
            }
        });
        Ok(ResourceSerde {
            serializer: serialize_fn,
            deserializer: deserialize_fn,
        })
    }

    pub fn register_type(
        &mut self,
        resource_type: &ResourceType,
        resource_serde: ResourceSerde,
    ) -> Result<(), RegistryError> {
        match self
            .serde_store
            .insert(resource_type.clone(), resource_serde)
        {
            Some(_) => Err(RegistryError::TypeRegisteredAlready(format!(
                "Type id {:?} was already registered.",
                resource_type
            ))),
            None => Ok(()),
        }
    }

    pub fn serialize_resource(
        &mut self,
        resource: &dyn ResourceItem,
        resource_type: &ResourceType,
    ) -> Result<Value, RegistryError> {
        self.ensure_type_is_registered(resource_type)?;
        (self.serde_store[resource_type].serializer)(resource)
    }
    pub fn ensure_type_is_registered(
        &mut self,
        resource_type: &ResourceType,
    ) -> Result<(), RegistryError> {
        if !self.serde_store.contains_key(resource_type) {
            self.ensure_provider_is_registered(&resource_type.rprovider)?;
            let resource_serde = self.provider_store[&resource_type.rprovider]
                .lock()
                .unwrap()
                .get_resource_serde(self, resource_type)?;
            self.register_type(resource_type, resource_serde)?;
            if !self.serde_store.contains_key(resource_type) {
                return Err(RegistryError::TypeNotRegistered(format!(
                    "Provider: {}, Region: {}, Type: {}",
                    resource_type.rprovider.name,
                    resource_type.rprovider.region,
                    resource_type.rtype.name
                )));
            }
        }
        Ok(())
    }
    pub fn ensure_provider_is_registered(
        &mut self,
        resource_provider: &RProvider,
    ) -> Result<(), RegistryError> {
        if !self.provider_store.contains_key(resource_provider) {
            self.provider_store.insert(
                resource_provider.clone(),
                (self.provider_fatory)(resource_provider)?,
            );
            if !self.provider_store.contains_key(resource_provider) {
                return Err(RegistryError::ProviderNotFound(format!(
                    "Name: {}, Region: {}",
                    resource_provider.name, resource_provider.region
                )));
            }
        }
        Ok(())
    }
    pub fn deserialize_resource(
        &mut self,
        value: &Value,
        resource_type: &ResourceType,
    ) -> Result<Arc<dyn ResourceItem>, RegistryError> {
        self.ensure_type_is_registered(resource_type)?;
        (self.serde_store[resource_type].deserializer)(value)
    }
}

#[derive(Debug, Error)]
pub enum RegistryError {
    #[error("DowncastError, could not downcast to SharedResource")]
    DowncastError,
    #[error("SerializerNotRegistered, provider was not able to registry resource: {0}")]
    TypeNotRegistered(String),
    #[error("TypeRegisteredAlready, provider was not able to registry resource: {0}")]
    TypeRegisteredAlready(String),
    #[error("ProviderNotFound, Provider factory was not able to get provider {0}")]
    ProviderNotFound(String),
    #[error("LockResourceError, could not lock resource")]
    LockResourceError,
    #[error("SerializeError: could not serialize resource {0}")]
    SerializationError(#[from] serde_json::Error),
    #[error("Unknown Type to register {0}")]
    ResourceTypeNotSupported(String),
}
