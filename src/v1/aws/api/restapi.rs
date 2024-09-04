use std::{
    collections::HashMap,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use aws_sdk_apigateway::{
    operation::{
        create_rest_api::builders::CreateRestApiFluentBuilder, get_rest_api::GetRestApiOutput,
        update_rest_api::builders::UpdateRestApiFluentBuilder,
    },
    types::{
        builders::{EndpointConfigurationBuilder, PatchOperationBuilder},
        ApiKeySourceType, EndpointConfiguration, EndpointType, Op, RestApi,
    },
    Client,
};
use aws_sdk_s3::primitives::DateTime;
use serde::{Deserialize, Serialize};

use crate::prelude::AwsManager;
use crate::{
    prelude::{AwsResource, AwsResourceCreator},
    v1::manager::{ManagerError, ResourceManager},
};

pub type RestAPIInput = SerializableCreateRestAPIInput;
pub type RestAPIOutput = SerializableGetRestAPIOutput;
pub type RestAPIManager = AwsManager<RestAPIInput, RestAPIOutput, Client>;
pub type RestAPI<'a> = AwsResource<'a, RestAPIInput, RestAPIOutput>;

impl RestAPI<'_> {}

impl AwsResourceCreator for RestAPI<'_> {
    type Input = RestAPIInput;
    type Output = RestAPIOutput;
    fn r#type() -> crate::prelude::AwsType {
        crate::prelude::AwsType::RestAPI
    }
    fn manager(
        handle: &tokio::runtime::Handle,
        config: &aws_config::SdkConfig,
    ) -> std::sync::Arc<dyn ResourceManager<Self::Input, Self::Output>> {
        RestAPIManager::new(handle, Client::new(config), config).arc()
    }

    fn input_hook(_id: &str, _input: &mut Self::Input) {}
}
impl RestAPIManager {
    fn lookup(
        &self,
        id: Option<String>,
    ) -> Result<Option<RestAPIOutput>, crate::v1::manager::ManagerError> {
        self.handle.block_on(async {
            self.client
                .get_rest_api()
                .set_rest_api_id(id)
                .send()
                .await
                .map_err(|e| ManagerError::LookupFail(format!("{:?}", e.into_source())))
                .map(RestAPIOutput::from)
                .map(Some)
                .or_else(|e| match e {
                    ManagerError::LookupFail(ref msg) if msg.contains("not found") => Ok(None),
                    _ => Err(e),
                })
        })
    }
    fn lookup_by_name(
        &self,
        name: Option<String>,
    ) -> Result<Option<RestAPIOutput>, crate::v1::manager::ManagerError> {
        let api = self.handle.block_on(async {
            let mut next_token: Option<String> = None;
            loop {
                let mut request = self.client.get_rest_apis();
                if let Some(ref token) = next_token {
                    request = request.position(token);
                }
                match request.send().await {
                    Ok(response) => {
                        if let Some(mut items) = response.items {
                            for item in items.into_iter() {
                                if item.name == name {
                                    return Ok(item);
                                }
                            }
                        }
                        next_token = response.position;
                        if next_token.is_none() {
                            return Err(ManagerError::LookupFail("Rest API not found".to_string()));
                        }
                    }
                    Err(e) => {
                        return Err(ManagerError::LookupFail(format!("{:?}", e.into_source())));
                    }
                }
            }
        })?;
        Ok(Some(api.into()))
    }

    fn create(
        &self,
        input: &mut RestAPIInput,
    ) -> Result<RestAPIOutput, crate::v1::manager::ManagerError> {
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
                "Policy create but not found".to_string(),
            )),
            Err(e) => Err(e),
        }
    }

    fn delete(&self, latest: &RestAPIOutput) -> Result<bool, crate::v1::manager::ManagerError> {
        self.handle.block_on(async {
            self.client
                .delete_rest_api()
                .set_rest_api_id(latest.id.clone())
                .send()
                .await
                .map_err(|e| ManagerError::DeleteFail(format!("{:?}", e.into_source())))
                .map(|_| true)
        })
    }

    fn syncup(
        &self,
        _latest: &RestAPIOutput,
        input: &mut RestAPIInput,
    ) -> Result<Option<RestAPIOutput>, crate::v1::manager::ManagerError> {
        let aws_update = input
            .clone()
            .to_aws_update(&self.client, _latest.id.clone())
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
                "Policy create but not found".to_string(),
            )),
            Err(e) => Err(e),
        }
    }
}

impl ResourceManager<RestAPIInput, RestAPIOutput> for RestAPIManager {
    fn lookup(&self, latest: &RestAPIOutput) -> Result<Option<RestAPIOutput>, ManagerError> {
        let id = latest.id.clone();
        self.lookup(id)
    }

    fn lookup_by_input(&self, input: &RestAPIInput) -> Result<Option<RestAPIOutput>, ManagerError> {
        self.lookup_by_name(input.name.clone())
    }

    fn create(&self, input: &mut RestAPIInput) -> Result<RestAPIOutput, ManagerError> {
        self.create(input)
    }

    fn delete(&self, latest: &RestAPIOutput) -> Result<bool, ManagerError> {
        self.delete(latest)
    }

    fn syncup(
        &self,
        latest: &RestAPIOutput,
        input: &mut RestAPIInput,
    ) -> Result<Option<RestAPIOutput>, ManagerError> {
        self.syncup(latest, input)
    }
}

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct SerializableCreateRestAPIInput {
    pub name: Option<String>,
    pub description: Option<String>,
    pub version: Option<String>,
    pub clone_from: Option<String>,
    pub binary_media_types: Option<Vec<String>>,
    pub minimum_compression_size: Option<i32>,
    pub api_key_source: Option<SerializableApiKeySourceType>,
    pub endpoint_configuration: Option<SerializableEndpointConfiguration>,
    pub policy: Option<String>,
    pub tags: Option<HashMap<String, String>>,
    pub disable_execute_api_endpoint: Option<bool>,
}

fn convert_datetime_to_system_time(datetime: DateTime) -> SystemTime {
    UNIX_EPOCH + Duration::new(datetime.secs() as u64, datetime.subsec_nanos())
}

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub enum SerializableApiKeySourceType {
    #[default]
    Authorizer,
    Header,
    Unknown,
}

impl From<SerializableApiKeySourceType> for ApiKeySourceType {
    fn from(api_key_source: SerializableApiKeySourceType) -> Self {
        match api_key_source {
            SerializableApiKeySourceType::Authorizer => ApiKeySourceType::Authorizer,
            SerializableApiKeySourceType::Header => ApiKeySourceType::Header,
            _ => ApiKeySourceType::Authorizer,
        }
    }
}
impl From<ApiKeySourceType> for SerializableApiKeySourceType {
    fn from(api_key_source: ApiKeySourceType) -> Self {
        match api_key_source {
            ApiKeySourceType::Authorizer => SerializableApiKeySourceType::Authorizer,
            ApiKeySourceType::Header => SerializableApiKeySourceType::Header,
            _ => SerializableApiKeySourceType::Authorizer,
        }
    }
}

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct SerializableEndpointConfiguration {
    pub types: Option<Vec<SerializableEndpointType>>,
    pub vpc_endpoint_ids: Option<Vec<String>>,
}

impl From<SerializableEndpointConfiguration> for EndpointConfiguration {
    fn from(endpoint_configuration: SerializableEndpointConfiguration) -> Self {
        EndpointConfigurationBuilder::default()
            .set_types(
                endpoint_configuration
                    .types
                    .map(|types| types.into_iter().map(EndpointType::from).collect()),
            )
            .set_vpc_endpoint_ids(endpoint_configuration.vpc_endpoint_ids)
            .build()
    }
}
impl From<EndpointConfiguration> for SerializableEndpointConfiguration {
    fn from(endpoint_configuration: EndpointConfiguration) -> Self {
        SerializableEndpointConfiguration {
            types: endpoint_configuration.types.map(|types| {
                types
                    .into_iter()
                    .map(SerializableEndpointType::from)
                    .collect()
            }),
            vpc_endpoint_ids: endpoint_configuration.vpc_endpoint_ids,
        }
    }
}

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub enum SerializableEndpointType {
    #[default]
    Edge,
    Private,
    Regional,
    Unknown,
}

impl From<SerializableEndpointType> for EndpointType {
    fn from(endpoint_type: SerializableEndpointType) -> Self {
        match endpoint_type {
            SerializableEndpointType::Edge => EndpointType::Edge,
            SerializableEndpointType::Private => EndpointType::Private,
            SerializableEndpointType::Regional => EndpointType::Regional,
            _ => EndpointType::Edge,
        }
    }
}
impl From<EndpointType> for SerializableEndpointType {
    fn from(endpoint_type: EndpointType) -> Self {
        match endpoint_type {
            EndpointType::Edge => SerializableEndpointType::Edge,
            EndpointType::Private => SerializableEndpointType::Private,
            EndpointType::Regional => SerializableEndpointType::Regional,
            _ => SerializableEndpointType::Edge,
        }
    }
}

impl SerializableCreateRestAPIInput {
    pub fn to_aws_input(self, client: &Client) -> anyhow::Result<CreateRestApiFluentBuilder> {
        Ok(client
            .create_rest_api()
            .set_name(self.name)
            .set_description(self.description)
            .set_version(self.version)
            .set_clone_from(self.clone_from)
            .set_binary_media_types(self.binary_media_types)
            .set_minimum_compression_size(self.minimum_compression_size)
            .set_api_key_source(self.api_key_source.map(ApiKeySourceType::from))
            .set_endpoint_configuration(
                self.endpoint_configuration.map(EndpointConfiguration::from),
            )
            .set_policy(self.policy)
            .set_tags(self.tags)
            .set_disable_execute_api_endpoint(self.disable_execute_api_endpoint))
    }
    pub fn to_aws_update(
        self,
        client: &Client,
        id: Option<String>,
    ) -> anyhow::Result<UpdateRestApiFluentBuilder> {
        Ok(client
            .update_rest_api()
            .set_rest_api_id(id)
            .set_patch_operations(Some(vec![
                PatchOperationBuilder::default()
                    .set_op(Some(Op::Replace))
                    .set_path(Some("/name".to_string()))
                    .set_value(Some(self.name.unwrap_or_default()))
                    .build(),
                PatchOperationBuilder::default()
                    .set_op(Some(Op::Replace))
                    .set_path(Some("/description".to_string()))
                    .set_value(Some(self.description.unwrap_or_default()))
                    .build(),
                PatchOperationBuilder::default()
                    .set_op(Some(Op::Replace))
                    .set_path(Some("/version".to_string()))
                    .set_value(Some(self.version.unwrap_or_default()))
                    .build(),
            ])))
    }
}

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct SerializableGetRestAPIOutput {
    pub id: Option<String>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub created_date: Option<SystemTime>,
    pub version: Option<String>,
    pub warnings: Option<Vec<String>>,
    pub binary_media_types: Option<Vec<String>>,
    pub minimum_compression_size: Option<i32>,
    pub api_key_source: Option<SerializableApiKeySourceType>,
    pub endpoint_configuration: Option<SerializableEndpointConfiguration>,
    pub policy: Option<String>,
    pub tags: Option<HashMap<String, String>>,
    pub disable_execute_api_endpoint: bool,
    pub root_resource_id: Option<String>,
}

impl From<RestApi> for SerializableGetRestAPIOutput {
    fn from(api: RestApi) -> Self {
        SerializableGetRestAPIOutput {
            id: api.id,
            name: api.name,
            description: api.description,
            created_date: api.created_date.map(convert_datetime_to_system_time),
            version: api.version,
            warnings: api.warnings,
            binary_media_types: api.binary_media_types,
            minimum_compression_size: api.minimum_compression_size,
            api_key_source: api.api_key_source.map(SerializableApiKeySourceType::from),
            endpoint_configuration: api
                .endpoint_configuration
                .map(SerializableEndpointConfiguration::from),
            policy: api.policy,
            tags: api.tags,
            disable_execute_api_endpoint: api.disable_execute_api_endpoint,
            root_resource_id: api.root_resource_id,
        }
    }
}

impl From<GetRestApiOutput> for SerializableGetRestAPIOutput {
    fn from(output: GetRestApiOutput) -> Self {
        SerializableGetRestAPIOutput {
            id: output.id,
            name: output.name,
            description: output.description,
            created_date: output.created_date.map(convert_datetime_to_system_time),
            version: output.version,
            warnings: output.warnings,
            binary_media_types: output.binary_media_types,
            minimum_compression_size: output.minimum_compression_size,
            api_key_source: output
                .api_key_source
                .map(SerializableApiKeySourceType::from),
            endpoint_configuration: output
                .endpoint_configuration
                .map(SerializableEndpointConfiguration::from),
            policy: output.policy,
            tags: output.tags,
            disable_execute_api_endpoint: output.disable_execute_api_endpoint,
            root_resource_id: output.root_resource_id,
        }
    }
}
