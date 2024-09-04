use std::collections::HashMap;

use aws_sdk_apigateway::{
    operation::{
        create_deployment::builders::CreateDeploymentFluentBuilder,
        get_deployment::GetDeploymentOutput, get_integration::GetIntegrationOutput,
        put_integration::builders::PutIntegrationFluentBuilder,
        update_deployment::builders::UpdateDeploymentFluentBuilder,
        update_integration::builders::UpdateIntegrationFluentBuilder,
    },
    types::{
        builders::DeploymentCanarySettingsBuilder, CacheClusterSize, DeploymentCanarySettings,
        MethodSnapshot,
    },
    Client,
};
use serde::{Deserialize, Serialize};

use crate::prelude::AwsManager;
use crate::{
    prelude::{AwsResource, AwsResourceCreator},
    v1::manager::{ManagerError, ResourceManager},
};

pub type RestAPIDeploymentInput = SerializableCreateDeploymentInput;
pub type RestAPIDeploymentOutput = SerializableGetDeploymentOutput;
pub type RestAPIDeploymentManager =
    AwsManager<RestAPIDeploymentInput, RestAPIDeploymentOutput, Client>;
pub type RestAPIDeployment<'a> = AwsResource<'a, RestAPIDeploymentInput, RestAPIDeploymentOutput>;

impl RestAPIDeployment<'_> {}

impl AwsResourceCreator for RestAPIDeployment<'_> {
    type Input = RestAPIDeploymentInput;
    type Output = RestAPIDeploymentOutput;
    fn r#type() -> crate::prelude::AwsType {
        crate::prelude::AwsType::RestAPIDeployment
    }
    fn manager(
        handle: &tokio::runtime::Handle,
        config: &aws_config::SdkConfig,
    ) -> std::sync::Arc<dyn ResourceManager<Self::Input, Self::Output>> {
        RestAPIDeploymentManager::new(handle, Client::new(config), config).arc()
    }

    fn input_hook(_id: &str, _input: &mut Self::Input) {}
}
impl RestAPIDeploymentManager {
    fn lookup(
        &self,
        rest_api_id: Option<String>,
        deployment_id: Option<String>,
    ) -> Result<Option<RestAPIDeploymentOutput>, crate::v1::manager::ManagerError> {
        self.handle.block_on(async {
            self.client
                .get_deployment()
                .set_rest_api_id(rest_api_id.clone())
                .set_deployment_id(deployment_id.clone())
                .send()
                .await
                .map_err(|e| ManagerError::LookupFail(format!("{:?}", e.into_source())))
                .map(RestAPIDeploymentOutput::from)
                .map(|mut r| {
                    r.rest_api_id = rest_api_id;
                    r
                })
                .map(Some)
                .or_else(|e| match e {
                    ManagerError::LookupFail(ref msg) if msg.contains("NotFound") => Ok(None),
                    _ => Err(e),
                })
        })
    }

    fn lookup_by_stage(
        &self,
        rest_api_id: Option<String>,
        stage: Option<String>,
    ) -> Result<Option<RestAPIDeploymentOutput>, crate::v1::manager::ManagerError> {
        let api = self.handle.block_on(async {
            let mut next_token: Option<String> = None;
            loop {
                let mut request = self
                    .client
                    .get_deployments()
                    .set_rest_api_id(rest_api_id.clone())
                    .set

                if let Some(ref token) = next_token {
                    request = request.position(token);
                }
                match request.send().await {
                    Ok(response) => {
                        if let Some(mut items) = response.items {
                            for item in items.into_iter() {
                                println!("{:?} with {:?}", item.path, path);
                                if item. == path {
                                    return Ok(Some(item.into()));
                                }
                            }
                        }
                        next_token = response.position;
                        if next_token.is_none() {
                            return Ok(None);
                        }
                    }
                    Err(e) => {
                        return Err(ManagerError::LookupFail(format!("{:?}", e.into_source())));
                    }
                }
            }
        })?;
        Ok(api)
    }

    fn create(
        &self,
        input: &mut RestAPIDeploymentInput,
    ) -> Result<RestAPIDeploymentOutput, crate::v1::manager::ManagerError> {
        let aws_input = input
            .clone()
            .to_aws_input(&self.client)
            .map_err(|e| ManagerError::CreateFail(format!("{:?}", e.root_cause())))?;
        self.handle.block_on(async {
            aws_input
                .send()
                .await
                .map_err(|e| ManagerError::CreateFail(format!("{:?}", e.into_source())))
        })?;
        match self.lookup_by_input(input) {
            Ok(Some(api)) => Ok(api),
            Ok(None) => Err(ManagerError::CreateFail(
                "Integration created but not found".to_string(),
            )),
            Err(e) => Err(e),
        }
    }

    fn delete(
        &self,
        latest: &RestAPIDeploymentOutput,
    ) -> Result<bool, crate::v1::manager::ManagerError> {
        self.handle.block_on(async {
            self.client
                .delete_integration()
                .set_http_method(latest.http_method.clone())
                .set_resource_id(latest.resource_id.clone())
                .set_rest_api_id(latest.rest_api_id.clone())
                .send()
                .await
                .map_err(|e| ManagerError::DeleteFail(format!("{:?}", e.into_source())))
                .map(|_| true)
        })
    }

    fn syncup(
        &self,
        _latest: &RestAPIDeploymentOutput,
        input: &mut RestAPIDeploymentInput,
    ) -> Result<Option<RestAPIDeploymentOutput>, crate::v1::manager::ManagerError> {
        let aws_update = input
            .clone()
            .to_aws_update(&self.client)
            .map_err(|e| ManagerError::CreateFail(format!("{:?}", e.root_cause())))?;
        self.handle.block_on(async {
            aws_update
                .send()
                .await
                .map_err(|e| ManagerError::UpdateFail(format!("{:?}", e.into_source())))
        })?;
        match self.lookup_by_input(input) {
            Ok(Some(result)) => Ok(Some(result)),
            Ok(None) => Err(ManagerError::CreateFail(
                "Integration modified but not found".to_string(),
            )),
            Err(e) => Err(e),
        }
    }
}

impl ResourceManager<RestAPIDeploymentInput, RestAPIDeploymentOutput> for RestAPIDeploymentManager {
    fn lookup(
        &self,
        latest: &RestAPIDeploymentOutput,
    ) -> Result<Option<RestAPIDeploymentOutput>, ManagerError> {
        self.lookup(
            latest.rest_api_id.clone(),
            latest.resource_id.clone(),
            latest.http_method.clone(),
        )
    }

    fn lookup_by_input(
        &self,
        input: &RestAPIDeploymentInput,
    ) -> Result<Option<RestAPIDeploymentOutput>, ManagerError> {
        self.lookup(
            input.rest_api_id.clone(),
            input.resource_id.clone(),
            input.http_method.clone(),
        )
    }

    fn create(
        &self,
        input: &mut RestAPIDeploymentInput,
    ) -> Result<RestAPIDeploymentOutput, ManagerError> {
        self.create(input)
    }

    fn delete(&self, latest: &RestAPIDeploymentOutput) -> Result<bool, ManagerError> {
        self.delete(latest)
    }

    fn syncup(
        &self,
        latest: &RestAPIDeploymentOutput,
        input: &mut RestAPIDeploymentInput,
    ) -> Result<Option<RestAPIDeploymentOutput>, ManagerError> {
        self.syncup(latest, input)
    }
}

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct SerializableCreateDeploymentInput {
    pub rest_api_id: Option<String>,
    pub stage_name: Option<String>,
    pub stage_description: Option<String>,
    pub description: Option<String>,
    pub cache_cluster_enabled: Option<bool>,
    pub cache_cluster_size: Option<SerializableCacheClusterSize>,
    pub variables: Option<HashMap<String, String>>,
    pub canary_settings: Option<SerializableDeploymentCanarySettings>,
    pub tracing_enabled: Option<bool>,
}

#[derive(Default, Serialize, Deserialize, Debug, Clone)]

pub enum SerializableCacheClusterSize {
    #[default]
    Size0Point5Gb,
    Size1Point6Gb,
    Size118Gb,
    Size13Point5Gb,
    Size237Gb,
    Size28Point4Gb,
    Size58Point2Gb,
    Size6Point1Gb,
    Unknown,
}
impl From<CacheClusterSize> for SerializableCacheClusterSize {
    fn from(size: CacheClusterSize) -> Self {
        match size {
            CacheClusterSize::Size0Point5Gb => SerializableCacheClusterSize::Size0Point5Gb,
            CacheClusterSize::Size1Point6Gb => SerializableCacheClusterSize::Size1Point6Gb,
            CacheClusterSize::Size118Gb => SerializableCacheClusterSize::Size118Gb,
            CacheClusterSize::Size13Point5Gb => SerializableCacheClusterSize::Size13Point5Gb,
            CacheClusterSize::Size237Gb => SerializableCacheClusterSize::Size237Gb,
            CacheClusterSize::Size28Point4Gb => SerializableCacheClusterSize::Size28Point4Gb,
            CacheClusterSize::Size58Point2Gb => SerializableCacheClusterSize::Size58Point2Gb,
            CacheClusterSize::Size6Point1Gb => SerializableCacheClusterSize::Size6Point1Gb,
            _ => SerializableCacheClusterSize::Unknown,
        }
    }
}
impl From<SerializableCacheClusterSize> for CacheClusterSize {
    fn from(size: SerializableCacheClusterSize) -> Self {
        match size {
            SerializableCacheClusterSize::Size0Point5Gb => CacheClusterSize::Size0Point5Gb,
            SerializableCacheClusterSize::Size1Point6Gb => CacheClusterSize::Size1Point6Gb,
            SerializableCacheClusterSize::Size118Gb => CacheClusterSize::Size118Gb,
            SerializableCacheClusterSize::Size13Point5Gb => CacheClusterSize::Size13Point5Gb,
            SerializableCacheClusterSize::Size237Gb => CacheClusterSize::Size237Gb,
            SerializableCacheClusterSize::Size28Point4Gb => CacheClusterSize::Size28Point4Gb,
            SerializableCacheClusterSize::Size58Point2Gb => CacheClusterSize::Size58Point2Gb,
            SerializableCacheClusterSize::Size6Point1Gb => CacheClusterSize::Size6Point1Gb,
            SerializableCacheClusterSize::Unknown => CacheClusterSize::Size0Point5Gb,
        }
    }
}
#[derive(Default, Serialize, Deserialize, Debug, Clone)]

pub struct SerializableDeploymentCanarySettings {
    pub percent_traffic: f64,
    pub stage_variable_overrides: Option<HashMap<String, String>>,
    pub use_stage_cache: bool,
}

impl From<DeploymentCanarySettings> for SerializableDeploymentCanarySettings {
    fn from(settings: DeploymentCanarySettings) -> Self {
        Self {
            percent_traffic: settings.percent_traffic,
            stage_variable_overrides: settings.stage_variable_overrides,
            use_stage_cache: settings.use_stage_cache,
        }
    }
}
impl From<SerializableDeploymentCanarySettings> for DeploymentCanarySettings {
    fn from(settings: SerializableDeploymentCanarySettings) -> Self {
        DeploymentCanarySettingsBuilder::default()
            .percent_traffic(settings.percent_traffic)
            .set_stage_variable_overrides(settings.stage_variable_overrides)
            .use_stage_cache(settings.use_stage_cache)
            .build()
    }
}

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct SerializableGetDeploymentOutput {
    pub rest_api_id: Option<String>,
    pub id: Option<String>,
    pub description: Option<String>,
    pub api_summary: Option<HashMap<String, HashMap<String, SerializableMethodSnapshot>>>,
}

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct SerializableMethodSnapshot {
    pub authorization_type: Option<String>,
    pub api_key_required: bool,
}
impl From<MethodSnapshot> for SerializableMethodSnapshot {
    fn from(snapshot: MethodSnapshot) -> Self {
        Self {
            authorization_type: snapshot.authorization_type,
            api_key_required: snapshot.api_key_required,
        }
    }
}

impl SerializableCreateDeploymentInput {
    pub fn to_aws_input(self, client: &Client) -> anyhow::Result<CreateDeploymentFluentBuilder> {
        Ok(client
            .create_deployment()
            .set_rest_api_id(self.rest_api_id)
            .set_stage_name(self.stage_name)
            .set_stage_description(self.stage_description)
            .set_description(self.description)
            .set_cache_cluster_enabled(self.cache_cluster_enabled)
            .set_cache_cluster_size(self.cache_cluster_size.map(CacheClusterSize::from))
            .set_variables(self.variables)
            .set_canary_settings(self.canary_settings.map(DeploymentCanarySettings::from))
            .set_tracing_enabled(self.tracing_enabled))
    }
    pub fn to_aws_update(self, client: &Client) -> anyhow::Result<UpdateDeploymentFluentBuilder> {
        //TODO
        Ok(client.update_deployment().set_rest_api_id(self.rest_api_id))
    }
}

impl From<GetDeploymentOutput> for SerializableGetDeploymentOutput {
    fn from(output: GetDeploymentOutput) -> Self {
        Self {
            rest_api_id: None,
            id: output.id,
            description: output.description,
            api_summary: output.api_summary.map(|summary| {
                summary
                    .into_iter()
                    .map(|(k, v)| {
                        (
                            k,
                            v.into_iter()
                                .map(|(k, v)| (k, SerializableMethodSnapshot::from(v)))
                                .collect(),
                        )
                    })
                    .collect()
            }),
        }
    }
}
