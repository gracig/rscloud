use std::{
    borrow::Borrow,
    collections::{HashMap, HashSet},
    fmt,
    sync::{Arc, Mutex},
};

use daggy::{stable_dag::StableDag, NodeIndex};
use petgraph::algo::toposort;
use thiserror::Error;

use super::{
    datastore::{Datastore, DatastoreError},
    manager::ResourceManager,
    registry::{ResourceType, SharedRegistry},
    resource::{Resource, ResourceError, ResourceState, SharedResource},
};
#[derive(Clone, Default)]
pub struct SharedPlan {
    inner: Arc<Mutex<Plan>>,
}
impl SharedPlan {
    pub fn resource<Input: Clone + 'static + fmt::Debug, Output: Clone + 'static + fmt::Debug>(
        &self,
        rtype: ResourceType,
        mngr: Arc<dyn ResourceManager<Input, Output>>,
        id: &str,
        state: ResourceState,
        input: Input,
    ) -> Result<SharedResource<Input, Output>, PlanError> {
        self.inner
            .lock()
            .unwrap()
            .add_resource(rtype, mngr, id, state, input)
    }
    pub fn apply(
        &self,
        datastore: &mut Datastore,
        registry: &SharedRegistry,
    ) -> Result<(), PlanError> {
        self.inner.lock().unwrap().apply(datastore, registry)
    }
    pub fn destroy(
        &self,
        datastore: &mut Datastore,
        registry: &SharedRegistry,
    ) -> Result<(), PlanError> {
        self.inner.lock().unwrap().destroy(datastore, registry)
    }
}

pub struct Plan {
    resources: HashMap<String, Arc<dyn ResourceItem>>,
}
impl Default for Plan {
    fn default() -> Self {
        println!("--- Creating plan ---");
        Self {
            resources: HashMap::new(),
        }
    }
}
impl Plan {
    pub fn add_resource<
        Input: Clone + 'static + fmt::Debug,
        Output: Clone + 'static + fmt::Debug,
    >(
        &mut self,
        resource_type: ResourceType,
        manager: Arc<dyn ResourceManager<Input, Output>>,
        id: &str,
        state: ResourceState,
        input: Input,
    ) -> Result<SharedResource<Input, Output>, PlanError> {
        if self.resources.contains_key(id) {
            return Err(PlanError::ResourceAlreadyExists(id.to_owned()));
        }
        let resource = SharedResource::new_resource(resource_type, manager, id, input, state);
        self.resources.insert(
            id.to_owned(),
            Arc::new(resource.clone()) as Arc<dyn ResourceItem>,
        );
        Ok(resource)
    }
    pub fn apply(
        &self,
        datastore: &mut Datastore,
        registry: &SharedRegistry,
    ) -> Result<(), PlanError> {
        println!("--- Applying plan ---");
        self.toposort().and_then(|sorted| {
            self.apply_absent(datastore, registry, &sorted)
                .and(self.apply_present(datastore, registry, &sorted))
        })
    }

    fn apply_present(
        &self,
        datastore: &mut Datastore,
        registry: &SharedRegistry,
        sorted: &[Arc<dyn ResourceItem>],
    ) -> Result<(), PlanError> {
        println!("--- Ensuring resources are present ---");

        for resource in sorted.iter() {
            let id = resource.id();
            if let ResourceState::Present = resource.state() {
                //Retrieve the latest resource version from datastore
                let latest = if datastore.contains(&id) {
                    datastore
                        .get(registry, &id)
                        .map_err(PlanError::DatastoreError)?
                } else {
                    None
                };
                //Ensure resource is present in the cloud, by creating or modifying it
                resource
                    .apply_bindings()
                    .and(resource.ensure_present(latest))
                    .map_err(PlanError::ResourceError)?;
                //Save the resource back to the datastore
                datastore
                    .insert_resource(registry, resource.borrow())
                    .map_err(PlanError::DatastoreError)?;
            }
        }
        Ok(())
    }

    fn apply_absent(
        &self,
        datastore: &mut Datastore,
        registry: &SharedRegistry,
        sorted: &[Arc<dyn ResourceItem>],
    ) -> Result<(), PlanError> {
        println!("--- Ensuring planned resources are absent---");
        sorted
            .iter()
            .rev() //The sorted list are reversed to delete children before parents
            .try_fold(vec![], |mut to_be_created, resource| {
                //Delete the resources from this plan with state marked as absent
                match resource.state() {
                    ResourceState::Absent => {
                        let id = resource.id();
                        if datastore.contains(&id) {
                            let latest = datastore
                                .get(registry, &id)
                                .map_err(PlanError::DatastoreError)?;
                            if let Some(latest) = latest {
                                latest.ensure_absent().map_err(PlanError::ResourceError)?;
                                datastore.remove(&id);
                            }
                        }
                    }
                    ResourceState::Present => to_be_created.push(resource.id()),
                }
                Ok(to_be_created)
            })
            .and_then(|to_be_created| {
                println!("--- Ensuring missing resources are absent---");
                self.delete_ids(
                    datastore
                        .keys()
                        .into_iter()
                        .filter(|item| !to_be_created.contains(item))
                        .collect(),
                    datastore,
                    registry,
                )
            })
    }
    fn delete_ids(
        &self,
        ids: Vec<String>,
        datastore: &mut Datastore,
        registry: &SharedRegistry,
    ) -> Result<(), PlanError> {
        let to_delete = ids.into_iter().try_fold(vec![], |mut acc, key| {
            //Retrieve the resource from the datastore
            if let Some(resource) = datastore
                .get(registry, &key)
                .map_err(PlanError::DatastoreError)?
            {
                acc.push(resource)
            }
            Ok(acc)
        })?;
        let sorted = my_toposort(&to_delete)?;
        sorted.into_iter().rev().try_for_each(|item| {
            let key = item.id();
            //Retrieve the resource from the datastore
            if let Some(resource) = datastore
                .get(registry, &key)
                .map_err(PlanError::DatastoreError)?
            {
                //delete the resource
                resource.ensure_absent().map_err(PlanError::ResourceError)?;
            }
            datastore.remove(&key);
            Ok(())
        })
    }

    pub fn destroy(
        &self,
        datastore: &mut Datastore,
        registry: &SharedRegistry,
    ) -> Result<(), PlanError> {
        // Find items from datastore to be deleted
        self.delete_ids(datastore.keys(), datastore, registry)
    }

    fn toposort(&self) -> Result<Vec<Arc<dyn ResourceItem + 'static>>, PlanError> {
        my_toposort(
            self.resources
                .values()
                .map(|r| Arc::clone(r) as Arc<dyn ResourceItem>)
                .collect::<Vec<_>>()
                .as_slice(),
        )
    }
}

fn my_toposort(items: &[Arc<dyn ResourceItem>]) -> Result<Vec<Arc<dyn ResourceItem>>, PlanError> {
    println!("--- Sorting components based on their dependencies ---");
    let mut idx_id_map = HashMap::<String, NodeIndex>::new();
    let mut dag = StableDag::<Arc<dyn ResourceItem>, u32, u32>::new();
    for resource in items {
        let idx = dag.add_node(Arc::clone(resource));
        idx_id_map.insert(resource.id(), idx);
    }
    for resource in items {
        for dep in resource.dependencies() {
            let dep_idx = idx_id_map
                .get(&dep)
                .ok_or(PlanError::DependencyNotFound(dep.to_string()))?;
            dag.add_edge(*dep_idx, idx_id_map[&resource.id()], 0)
                .map_err(|err| PlanError::DagCreationError(format!("{:?}", err)))?;
        }
    }
    let sorted_indexes = toposort(dag.graph(), None)
        .map_err(|err| PlanError::DagCreationError(format!("{:?}", err)))?;
    let sorted_resources: Vec<Arc<dyn ResourceItem>> = sorted_indexes
        .into_iter()
        .map(|idx| Arc::clone(dag.node_weight(idx).unwrap()))
        .collect();
    Ok(sorted_resources)
}
pub trait ResourceItem: fmt::Debug {
    fn as_any(&self) -> &dyn std::any::Any;
    fn resource_type(&self) -> ResourceType;
    fn id(&self) -> String;
    fn name(&self) -> String;
    fn state(&self) -> ResourceState;
    fn set_state(&self, state: ResourceState);
    fn dependencies(&self) -> HashSet<String>;
    fn ensure_present(&self, latest: Option<Arc<dyn ResourceItem>>) -> Result<(), ResourceError>;
    fn ensure_absent(&self) -> Result<bool, ResourceError>;
    fn apply_bindings(&self) -> Result<(), ResourceError>;
}

pub fn item_as_resource<Input: Clone + 'static, Output: Clone + 'static>(
    item: &dyn ResourceItem,
) -> Resource<Input, Output> {
    item.as_any()
        .downcast_ref::<SharedResource<Input, Output>>()
        .unwrap()
        .resource
        .lock()
        .unwrap()
        .clone()
}

#[derive(Debug, Error)]
pub enum PlanError {
    #[error("Resource {0} already exists")]
    ResourceAlreadyExists(String),
    #[error("Dependency {0} not found")]
    DependencyNotFound(String),
    #[error("Dag creation error")]
    DagCreationError(String),
    #[error("Resource error")]
    ResourceError(ResourceError),
    #[error("Datastore error")]
    DatastoreError(DatastoreError),
    #[error("Generic error")]
    AnyhowError(anyhow::Error),
}
