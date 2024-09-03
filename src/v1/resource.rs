use std::{
    collections::HashSet,
    fmt,
    sync::{Arc, Mutex, MutexGuard},
};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use super::{
    manager::{ManagerError, ResourceManager},
    plan::{item_as_resource, ResourceItem},
    registry::ResourceType,
};

#[derive(Debug, Clone)]
pub struct SharedResource<Input: Clone, Output: Clone> {
    pub resource: Arc<Mutex<Resource<Input, Output>>>,
}

impl<Input: Clone + 'static + fmt::Debug, Output: Clone + 'static + fmt::Debug> ResourceItem
    for SharedResource<Input, Output>
{
    fn id(&self) -> String {
        self.resource.lock().unwrap().id.clone()
    }
    fn name(&self) -> String {
        self.resource.lock().unwrap().name.clone()
    }
    fn resource_type(&self) -> ResourceType {
        self.resource.lock().unwrap().resource_type.clone()
    }
    fn state(&self) -> ResourceState {
        self.resource.lock().unwrap().state.clone()
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn set_state(&self, state: ResourceState) {
        self.resource.lock().unwrap().state = state;
    }
    fn dependencies(&self) -> HashSet<String> {
        self.resource.lock().unwrap().dependencies.clone()
    }
    fn ensure_present(&self, latest: Option<Arc<dyn ResourceItem>>) -> Result<(), ResourceError> {
        self.resource.lock().unwrap().ensure_present(latest)
    }
    fn ensure_absent(&self) -> Result<bool, ResourceError> {
        self.resource.lock().unwrap().ensure_absent()
    }

    fn apply_bindings(&self) -> Result<(), ResourceError> {
        self.resource.lock().unwrap().apply_bindings()
    }
}

impl<Input: Clone + 'static, Output: Clone + 'static> SharedResource<Input, Output> {
    pub fn new_resource(
        resource_type: ResourceType,
        manager: Arc<dyn ResourceManager<Input, Output>>,
        id: impl ToString,
        input: Input,
        state: ResourceState,
    ) -> Self {
        Self {
            resource: Arc::new(Mutex::new(Resource::new(
                resource_type,
                manager,
                id,
                input,
                state,
            ))),
        }
    }
    pub fn new(resource: Resource<Input, Output>) -> Self {
        Self {
            resource: Arc::new(Mutex::new(resource)),
        }
    }
    pub fn bind<I2: 'static + Clone, O2: 'static + Clone>(
        &self,
        dependency: &SharedResource<I2, O2>,
        bind_fn: impl Fn(&mut Input, &O2) + 'static,
    ) -> Result<(), ResourceError> {
        let dep = dependency.clone();
        let other_id = dep.id()?;
        let other_state = dep.state()?;
        self.resource
            .lock()
            .unwrap()
            .dependencies
            .insert(other_id.clone());
        self.resource
            .lock()
            .map_err(|err| ResourceError::LockFail(err.to_string()))
            .map(|mut inner| {
                println!("Binding Output[{}] to Input[{}]", other_id, inner.id);
                if let ResourceState::Absent = other_state {
                    inner.state = ResourceState::Absent;
                    println!(
                        "Adding Resource[{}] with state [{:?}] to the plan",
                        inner.id, inner.state
                    );
                }
                inner.bindings.push(Bind {
                    dep_id: other_id.clone(),
                    dep_fn: Arc::new({
                        move |input: &mut Input| {
                            dep.with_output(|output| {
                                bind_fn(input, output);
                            })
                        }
                    }),
                })
            })
    }
    fn lock(&self) -> Result<MutexGuard<Resource<Input, Output>>, ResourceError> {
        self.resource
            .lock()
            .map_err(|err| ResourceError::LockFail(err.to_string()))
    }
    pub fn id(&self) -> Result<String, ResourceError> {
        self.lock().map(|inner| inner.id.clone())
    }
    pub fn state(&self) -> Result<ResourceState, ResourceError> {
        self.lock().map(|inner| inner.state.clone())
    }
    pub fn output(&self) -> Result<Option<Output>, ResourceError> {
        Ok(self.lock()?.output.clone())
    }

    pub fn with_output(&self, mut apply: impl FnMut(&Output)) -> Result<(), ResourceError> {
        self.lock().and_then(|inner| match &inner.output {
            Some(output) => {
                apply(output);
                Ok(())
            }
            None => Err(ResourceError::DependencyOutputIsMissing(
                "Output is not ready".to_string(),
            )),
        })
    }
}

#[derive(Clone, Serialize, Deserialize, Default)]
pub struct Resource<Input, Output> {
    pub resource_type: ResourceType, //Info that helps deserialize and serialize this resource
    pub id: String,
    #[serde(default)]
    pub name: String,
    pub input: Input,
    pub output: Option<Output>,
    pub state: ResourceState,
    pub dependencies: HashSet<String>,
    #[serde(skip, default = "default_bindings")]
    pub bindings: Vec<Bind<Input>>,
    #[serde(skip, default = "default_manager")]
    pub manager: Option<Arc<dyn ResourceManager<Input, Output>>>,
}
impl<Input: fmt::Debug, Output: fmt::Debug> fmt::Debug for Resource<Input, Output> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Resource")
            .field("resource_type", &self.resource_type)
            .field("id", &self.id)
            .field("input", &self.input)
            .field("output", &self.output)
            .field("state", &self.state)
            .field("dependencies", &self.dependencies)
            .field("bindings", &self.bindings)
            .finish()
    }
}

impl<Input: Clone + 'static, Output: Clone + 'static> Resource<Input, Output> {
    pub fn new(
        resource_type: ResourceType,
        manager: Arc<dyn ResourceManager<Input, Output>>,
        id: impl ToString,
        input: Input,
        state: ResourceState,
    ) -> Self {
        let name = id.to_string();
        let id = format!(
            "{}/{}/{}/{}",
            resource_type.rprovider.name,
            resource_type.rprovider.region,
            resource_type.rtype.name,
            &name
        );
        println!("New Resource[{}]", id);
        Self {
            id,
            name,
            resource_type,
            input,
            output: Default::default(),
            state,
            dependencies: Default::default(),
            bindings: Default::default(),
            manager: Some(manager.clone()),
        }
    }
    fn ensure_present(
        &mut self,
        latest: Option<Arc<dyn ResourceItem>>,
    ) -> Result<(), ResourceError> {
        let latest_output =
            latest.and_then(|latest| item_as_resource::<Input, Output>(latest.as_ref()).output);
        match self.manager.as_ref() {
            Some(manager) => {
                println!("Ensuring Resource[{}] is present", self.id);
                manager
                    .ensure_present(latest_output.as_ref(), &mut self.input)
                    .map(|output| {
                        println!("Resource Output[{}] is present", self.id);
                        self.output = Some(output)
                    })
                    .map_err(ResourceError::ManagerError)
            }
            None => Err(ResourceError::ManagerNotSet),
        }
    }

    fn ensure_absent(&mut self) -> Result<bool, ResourceError> {
        let output = match self.output.as_ref() {
            Some(output) => output,
            None => return Ok(false),
        };
        match self.manager.as_ref() {
            Some(manager) => {
                println!("Ensuring Resource[{}] is absent", self.id);
                manager
                    .ensure_absent(output)
                    .map_err(ResourceError::ManagerError)
                    .inspect(|_| println!("Resource[{}] is absent", self.id))
            }
            None => Err(ResourceError::ManagerNotSet),
        }
    }
    fn apply_bindings(&mut self) -> Result<(), ResourceError> {
        println!("--- Applying Resource[{}] bindings ---", self.id);

        let mut input = self.input.clone();
        for b in self.bindings.iter() {
            (b.dep_fn)(&mut input)?;
            println!(
                "Resource[{}].output to Resource[{}].input is bound",
                b.dep_id, self.id
            );
        }
        self.input = input;
        Ok(())
    }
}

pub fn default_manager<Input, Output>() -> Option<Arc<dyn ResourceManager<Input, Output>>> {
    None
}
pub fn default_bindings<Input>() -> Vec<Bind<Input>> {
    vec![]
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub enum ResourceState {
    Absent,
    #[default]
    Present,
}

#[derive(Clone)]
#[allow(clippy::type_complexity)]
pub struct Bind<Input> {
    dep_id: String,
    dep_fn: Arc<dyn Fn(&mut Input) -> Result<(), ResourceError>>,
}
impl<T: fmt::Debug> fmt::Debug for Bind<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Bind")
            .field("dep_id", &self.dep_id)
            .finish()
    }
}

impl<Input: Serialize, Output: Serialize> Resource<Input, Output> {
    pub fn serialize(&self) -> Value {
        serde_json::to_value(self).unwrap()
    }
}

#[derive(Debug, Error)]
pub enum ResourceError {
    #[error("BindingFail")]
    BindingFail(String),
    #[error("LockFail")]
    LockFail(String),
    #[error("DependencyOutputIsMissing: {0}")]
    DependencyOutputIsMissing(String),
    #[error("ManagerNotSet")]
    ManagerNotSet,
    #[error("ManagerError")]
    ManagerError(ManagerError),
}
