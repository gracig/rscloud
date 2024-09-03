use std::sync::{Arc, Mutex};

use tokio::runtime::Handle;

use super::{
    aws::AwsProvider,
    datastore::{Datastore, DatastoreError},
    plan::{PlanError, SharedPlan},
    registry::{RProvider, RegistryError, ResourceSerdeProvider, SharedRegistry},
    resource::ResourceError,
};

#[derive(Default)]
pub struct FPCloud {
    plan: SharedPlan,
    datastore: Datastore,
    registry: Option<SharedRegistry>,
}

impl FPCloud {
    pub fn init_registry(&mut self, handle: Handle) {
        let plan = self.plan.clone(); // Clone `plan` outside the closure
        self.registry = Some(SharedRegistry::new(Box::new(
            move |r| -> Result<Arc<Mutex<dyn ResourceSerdeProvider>>, RegistryError> {
                let handle = handle.clone();
                let r_clone = r.clone();
                if r.name == "aws" {
                    Ok(
                        Arc::new(Mutex::new(AwsProvider::new(&handle, plan.clone(), r_clone)))
                            as Arc<Mutex<dyn ResourceSerdeProvider>>,
                    )
                } else {
                    Err(RegistryError::ProviderNotFound(r.name.to_string()))
                }
            },
        )));
    }

    pub fn aws_provider(&self, handle: Handle, region: impl ToString) -> AwsProvider {
        AwsProvider::new(
            &handle,
            self.plan.clone(),
            RProvider {
                name: "aws".to_string(),
                region: region.to_string(),
            },
        )
    }
    pub fn apply(&mut self) -> Result<(), CloudError> {
        self.datastore
            .reload()
            .map_err(CloudError::DatastoreError)?;
        let result = match self.registry.as_ref() {
            Some(registry) => self
                .plan
                .apply(&mut self.datastore, registry)
                .map_err(CloudError::PlanError),
            None => Err(CloudError::RegistryNotInitialized),
        };
        self.datastore.save().map_err(CloudError::DatastoreError)?;
        result
    }
    pub fn destroy(&mut self) -> Result<(), CloudError> {
        self.datastore
            .reload()
            .map_err(CloudError::DatastoreError)?;
        let result = match self.registry.as_ref() {
            Some(registry) => self
                .plan
                .destroy(&mut self.datastore, registry)
                .map_err(CloudError::PlanError),
            None => Err(CloudError::RegistryNotInitialized),
        };
        self.datastore.save().map_err(CloudError::DatastoreError)?;
        result
    }
}

#[derive(Debug)]
pub enum CloudError {
    ResourceError(ResourceError),
    DatastoreError(DatastoreError),
    PlanError(PlanError),
    RegistryNotInitialized,
    LockError,
}

impl std::fmt::Display for CloudError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            CloudError::ResourceError(e) => write!(f, "ResourceError: {:?}", e),
            CloudError::PlanError(e) => write!(f, "ResourceError: {:?}", e),
            CloudError::LockError => write!(f, "LockError"),
            CloudError::RegistryNotInitialized => write!(f, "Registry not initialized"),
            CloudError::DatastoreError(e) => write!(f, "DatastoreError: {:?}", e),
        }
    }
}

impl std::error::Error for CloudError {}
