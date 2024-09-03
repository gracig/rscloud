pub mod ec2;
pub mod iam;
pub mod lambda;
pub mod s3;
pub mod scheduler;
pub mod sfn;

use self::s3::bucket::Bucket;

use super::{
    manager::{ManagerError, ResourceManager},
    plan::{PlanError, SharedPlan},
    registry::{
        RProvider, RType, Registry, RegistryError, ResourceSerde, ResourceSerdeProvider,
        ResourceType,
    },
    resource::{ResourceError, ResourceState, SharedResource},
};
use crate::prelude::*;
use aws_config::{BehaviorVersion, Region, SdkConfig};
use aws_sdk_sts::operation::get_caller_identity::GetCallerIdentityOutput;
use iam::{group::IAMGroup, user::IAMUser};
use scheduler::schedule::Schedule;
use serde::{Deserialize, Serialize};
use std::{fmt, marker::PhantomData, str::FromStr, sync::Arc};
use strum_macros::EnumString;
use tokio::runtime::Handle;

#[derive(Serialize, Deserialize, Debug, Clone, EnumString, strum_macros::Display)]
pub enum AwsType {
    #[strum(ascii_case_insensitive)]
    Vpc,
    Subnet,
    Function,
    Role,
    Policy,
    AttachRolePolicy,
    AttachGroupPolicy,
    AttachUserPolicy,
    StateMachine,
    Bucket,
    S3Object,
    Schedule,
    Group,
    User,
}
pub struct AwsProvider {
    plan: SharedPlan,
    handle: Handle,
    config: SdkConfig,
    rprovider: RProvider,
}
pub struct AwsManager<Input, Output, Client> {
    sts_client: aws_sdk_sts::Client,
    client: Client,
    handle: Handle,
    config: SdkConfig,
    _phantom: PhantomData<(Input, Output)>,
}
pub trait ArnProvider {
    fn arn(&self) -> Option<Vec<String>>;
}
#[derive(Clone)]
pub struct AwsResource<'a, Input: Clone, Output: Clone> {
    pub aws: &'a AwsProvider,
    pub inner: SharedResource<Input, Output>,
}
impl<'a, Input: Clone + 'static, Output: Clone + 'static> AwsResource<'a, Input, Output> {
    pub fn bind<I2: 'static + Clone, O2: 'static + Clone>(
        &self,
        dep: &AwsResource<'a, I2, O2>,
        bind_fn: impl Fn(&mut Input, &O2) + 'static,
    ) -> Result<(), ResourceError> {
        self.inner.bind(&dep.inner, bind_fn)
    }
}
pub trait AwsResourceCreator {
    type Input: Clone + fmt::Debug + 'static;
    type Output: Clone + fmt::Debug + 'static;
    fn r#type() -> AwsType;
    fn manager(
        handle: &Handle,
        config: &SdkConfig,
    ) -> Arc<dyn ResourceManager<Self::Input, Self::Output>>;
    fn input_hook(id: &str, input: &mut Self::Input);
    fn create(
        plan: &SharedPlan,
        handle: &Handle,
        config: &SdkConfig,
        rprovider: RProvider,
        id: &str,
        state: ResourceState,
        input: Self::Input,
    ) -> Result<SharedResource<Self::Input, Self::Output>, PlanError> {
        plan.resource(
            ResourceType {
                rprovider,
                rtype: RType {
                    name: Self::r#type().to_string(),
                },
            },
            Self::manager(handle, config),
            id,
            state,
            input,
        )
    }
}

impl ResourceSerdeProvider for AwsProvider {
    fn get_resource_serde(
        &self,
        r: &Registry,
        t: &ResourceType,
    ) -> Result<ResourceSerde, RegistryError> {
        let handle = &self.handle;
        let config = &self.config;
        match AwsType::from_str(t.rtype.name.as_str())
            .map_err(|_| RegistryError::ResourceTypeNotSupported(t.rtype.name.clone()))?
        {
            AwsType::Vpc => r.serde(Vpc::manager(handle, config)),
            AwsType::Subnet => r.serde(Subnet::manager(handle, config)),
            AwsType::Function => r.serde(Function::manager(handle, config)),
            AwsType::Role => r.serde(Role::manager(handle, config)),
            AwsType::StateMachine => r.serde(StateMachine::manager(handle, config)),
            AwsType::Policy => r.serde(Policy::manager(handle, config)),
            AwsType::AttachRolePolicy => r.serde(AttachRolePolicy::manager(handle, config)),
            AwsType::Bucket => r.serde(Bucket::manager(handle, config)),
            AwsType::S3Object => r.serde(S3Object::manager(handle, config)),
            AwsType::Schedule => r.serde(Schedule::manager(handle, config)),
            AwsType::Group => r.serde(IAMGroup::manager(handle, config)),
            AwsType::AttachGroupPolicy => r.serde(AttachGroupPolicy::manager(handle, config)),
            AwsType::User => r.serde(IAMUser::manager(handle, config)),
            AwsType::AttachUserPolicy => r.serde(AttachUserPolicy::manager(handle, config)),
        }
    }
}

impl<Input: Clone, Output: Clone, Client: 'static> AwsManager<Input, Output, Client>
where
    Input: 'static + Serialize + for<'de> Deserialize<'de>,
    Output: 'static + Serialize + for<'de> Deserialize<'de>,
    AwsManager<Input, Output, Client>: ResourceManager<Input, Output>,
{
    fn new(handle: &Handle, client: Client, config: &SdkConfig) -> Self {
        Self {
            client,
            handle: handle.clone(),
            config: config.clone(),
            sts_client: aws_sdk_sts::Client::new(config),
            _phantom: PhantomData,
        }
    }
    pub fn get_identity(&self) -> Result<GetCallerIdentityOutput, ManagerError> {
        self.handle.block_on(async {
            self.sts_client
                .get_caller_identity()
                .send()
                .await
                .map_err(|e| ManagerError::LookupFail(format!("{:?}", e.into_source())))
        })
    }
    pub fn get_region(&self) -> Result<String, ManagerError> {
        let region = self
            .config
            .region()
            .ok_or(ManagerError::LookupFail("Region not found".to_string()))?;
        Ok(region.to_string())
    }
    fn arc(self) -> Arc<Self> {
        Arc::new(self)
    }
}

unsafe impl Send for AwsProvider {}
impl AwsProvider {
    pub fn new(handle: &Handle, plan: SharedPlan, rprovider: RProvider) -> Self {
        handle.clone().block_on(async move {
            let region = Region::new(rprovider.clone().region);
            Self {
                handle: handle.clone(),
                plan,
                config: aws_config::defaults(BehaviorVersion::latest())
                    .region(region)
                    .load()
                    .await,
                rprovider,
            }
        })
    }
    pub fn resource<Aws: AwsResourceCreator>(
        &self,
        id: &str,
        state: ResourceState,
        mut input: Aws::Input,
    ) -> Result<AwsResource<Aws::Input, Aws::Output>, PlanError> {
        Aws::input_hook(id, &mut input);
        Ok(AwsResource {
            aws: self,
            inner: Aws::create(
                &self.plan,
                &self.handle,
                &self.config,
                self.rprovider.clone(),
                id,
                state,
                input,
            )?,
        })
    }
}
