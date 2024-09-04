use std::collections::HashMap;

use aws_sdk_apigateway::{
    operation::{
        create_resource::builders::CreateResourceFluentBuilder, get_resource::GetResourceOutput,
        update_resource::builders::UpdateResourceFluentBuilder,
    },
    types::{
        builders::PatchOperationBuilder, ConnectionType, ContentHandlingStrategy, Integration,
        IntegrationResponse, IntegrationType, Method, MethodResponse, Op, TlsConfig,
    },
    Client,
};
use serde::{Deserialize, Serialize};

use crate::prelude::AwsManager;
use crate::{
    prelude::{AwsResource, AwsResourceCreator},
    v1::manager::{ManagerError, ResourceManager},
};

pub type RestAPIResourceInput = SerializableResourceInput;
pub type RestAPIResourceOutput = SerializableGetResourceOutput;
pub type RestAPIResourceManager = AwsManager<RestAPIResourceInput, RestAPIResourceOutput, Client>;
pub type RestAPIResource<'a> = AwsResource<'a, RestAPIResourceInput, RestAPIResourceOutput>;

impl RestAPIResource<'_> {}

impl AwsResourceCreator for RestAPIResource<'_> {
    type Input = RestAPIResourceInput;
    type Output = RestAPIResourceOutput;
    fn r#type() -> crate::prelude::AwsType {
        crate::prelude::AwsType::RestAPI
    }
    fn manager(
        handle: &tokio::runtime::Handle,
        config: &aws_config::SdkConfig,
    ) -> std::sync::Arc<dyn ResourceManager<Self::Input, Self::Output>> {
        RestAPIResourceManager::new(handle, Client::new(config), config).arc()
    }

    fn input_hook(_id: &str, _input: &mut Self::Input) {}
}
impl RestAPIResourceManager {
    fn lookup(
        &self,
        rest_api_id: Option<String>,
        resource_id: Option<String>,
    ) -> Result<Option<RestAPIResourceOutput>, crate::v1::manager::ManagerError> {
        self.handle.block_on(async {
            self.client
                .get_resource()
                .set_rest_api_id(rest_api_id)
                .set_resource_id(resource_id)
                .send()
                .await
                .map_err(|e| ManagerError::LookupFail(format!("{:?}", e.into_source())))
                .map(RestAPIResourceOutput::from)
                .map(Some)
                .or_else(|e| match e {
                    ManagerError::LookupFail(ref msg) if msg.contains("NotFound") => Ok(None),
                    _ => Err(e),
                })
        })
    }
    fn lookup_by_path(
        &self,
        rest_api_id: Option<String>,
        path: Option<String>,
    ) -> Result<Option<RestAPIResourceOutput>, crate::v1::manager::ManagerError> {
        let api = self.handle.block_on(async {
            let mut next_token: Option<String> = None;
            loop {
                let mut request = self
                    .client
                    .get_resources()
                    .set_rest_api_id(rest_api_id.clone());
                if let Some(ref token) = next_token {
                    request = request.position(token);
                }
                match request.send().await {
                    Ok(response) => {
                        if let Some(mut items) = response.items {
                            for item in items.into_iter() {
                                if item.path == path {
                                    return Ok(item);
                                }
                            }
                        }
                        next_token = response.position;
                        if next_token.is_none() {
                            return Err(ManagerError::LookupFail("Resource not found".to_string()));
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
        input: &mut RestAPIResourceInput,
    ) -> Result<RestAPIResourceOutput, crate::v1::manager::ManagerError> {
        let aws_input = input
            .clone()
            .to_aws_input(&self.client)
            .map_err(|e| ManagerError::CreateFail(format!("{:?}", e.root_cause())))?;
        let response = self.handle.block_on(async {
            aws_input
                .send()
                .await
                .map_err(|e| ManagerError::CreateFail(format!("{:?}", e.into_source())))
        })?;
        let mut response = match self.lookup(input.parent_id.clone(), response.id.clone()) {
            Ok(Some(api)) => Ok(api),
            Ok(None) => Err(ManagerError::CreateFail(
                "Policy create but not found".to_string(),
            )),
            Err(e) => Err(e),
        }?;
        response.rest_api_id = input.rest_api_id.clone();
        Ok(response)
    }

    fn delete(
        &self,
        latest: &RestAPIResourceOutput,
    ) -> Result<bool, crate::v1::manager::ManagerError> {
        self.handle.block_on(async {
            self.client
                .delete_resource()
                .set_rest_api_id(latest.rest_api_id.clone())
                .set_resource_id(latest.id.clone())
                .send()
                .await
                .map_err(|e| ManagerError::DeleteFail(format!("{:?}", e.into_source())))
                .map(|_| true)
        })
    }

    fn syncup(
        &self,
        _latest: &RestAPIResourceOutput,
        input: &mut RestAPIResourceInput,
    ) -> Result<Option<RestAPIResourceOutput>, crate::v1::manager::ManagerError> {
        let aws_update = input
            .clone()
            .to_aws_update(&self.client, _latest.id.clone())
            .map_err(|e| ManagerError::CreateFail(format!("{:?}", e.root_cause())))?;
        let response = self.handle.block_on(async {
            aws_update
                .send()
                .await
                .map_err(|e| ManagerError::UpdateFail(format!("{:?}", e.into_source())))
        })?;
        let mut response = match self.lookup(input.rest_api_id.clone(), response.id.clone()) {
            Ok(Some(result)) => Ok(Some(result)),
            Ok(None) => Err(ManagerError::CreateFail(
                "Policy create but not found".to_string(),
            )),
            Err(e) => Err(e),
        }?;
        if let Some(response) = response.as_mut() {
            response.rest_api_id = input.rest_api_id.clone();
        }
        Ok(response)
    }
}

impl ResourceManager<RestAPIResourceInput, RestAPIResourceOutput> for RestAPIResourceManager {
    fn lookup(
        &self,
        latest: &RestAPIResourceOutput,
    ) -> Result<Option<RestAPIResourceOutput>, ManagerError> {
        self.lookup(latest.rest_api_id.clone(), latest.id.clone())
    }

    fn lookup_by_input(
        &self,
        input: &RestAPIResourceInput,
    ) -> Result<Option<RestAPIResourceOutput>, ManagerError> {
        self.lookup_by_path(input.rest_api_id.clone(), input.path_part.clone())
    }

    fn create(
        &self,
        input: &mut RestAPIResourceInput,
    ) -> Result<RestAPIResourceOutput, ManagerError> {
        self.create(input)
    }

    fn delete(&self, latest: &RestAPIResourceOutput) -> Result<bool, ManagerError> {
        self.delete(latest)
    }

    fn syncup(
        &self,
        latest: &RestAPIResourceOutput,
        input: &mut RestAPIResourceInput,
    ) -> Result<Option<RestAPIResourceOutput>, ManagerError> {
        self.syncup(latest, input)
    }
}

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct SerializableResourceInput {
    pub rest_api_id: Option<String>,
    pub parent_id: Option<String>,
    pub path_part: Option<String>,
}

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct SerializableGetResourceOutput {
    pub id: Option<String>,
    pub rest_api_id: Option<String>,
    pub parent_id: Option<String>,
    pub path_part: Option<String>,
    pub path: Option<String>,
    pub resource_methods: Option<HashMap<String, SerializableMethod>>,
}

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct SerializableMethod {
    pub http_method: Option<String>,
    pub authorization_type: Option<String>,
    pub authorizer_id: Option<String>,
    pub api_key_required: Option<bool>,
    pub request_validator_id: Option<String>,
    pub operation_name: Option<String>,
    pub request_parameters: Option<HashMap<String, bool>>,
    pub request_models: Option<HashMap<String, String>>,
    pub method_responses: Option<HashMap<String, SerializableMethodResponse>>,
    pub method_integration: Option<SerializableIntegration>,
    pub authorization_scopes: Option<Vec<String>>,
}
impl From<Method> for SerializableMethod {
    fn from(value: Method) -> Self {
        Self {
            http_method: value.http_method,
            authorization_type: value.authorization_type,
            authorizer_id: value.authorizer_id,
            api_key_required: value.api_key_required,
            request_validator_id: value.request_validator_id,
            operation_name: value.operation_name,
            request_parameters: value.request_parameters,
            request_models: value.request_models,
            method_responses: value.method_responses.map(|responses| {
                responses
                    .into_iter()
                    .map(|(key, value)| (key, SerializableMethodResponse::from(value)))
                    .collect()
            }),
            method_integration: value.method_integration.map(SerializableIntegration::from),
            authorization_scopes: value.authorization_scopes,
        }
    }
}

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct SerializableMethodResponse {
    pub status_code: Option<String>,
    pub response_parameters: Option<HashMap<String, bool>>,
    pub response_models: Option<HashMap<String, String>>,
}
impl From<MethodResponse> for SerializableMethodResponse {
    fn from(value: MethodResponse) -> Self {
        Self {
            status_code: value.status_code,
            response_parameters: value.response_parameters,
            response_models: value.response_models,
        }
    }
}

#[derive(Default, Serialize, Deserialize, Debug, Clone)]

pub struct SerializableIntegration {
    pub r#type: Option<SerializableIntegrationType>,
    pub http_method: Option<String>,
    pub uri: Option<String>,
    pub connection_type: Option<SerializableConnectionType>,
    pub connection_id: Option<String>,
    pub credentials: Option<String>,
    pub request_parameters: Option<HashMap<String, String>>,
    pub request_templates: Option<HashMap<String, String>>,
    pub passthrough_behavior: Option<String>,
    pub content_handling: Option<SerializableContentHandlingStrategy>,
    pub timeout_in_millis: i32,
    pub cache_namespace: Option<String>,
    pub cache_key_parameters: Option<Vec<String>>,
    pub integration_responses: Option<HashMap<String, SerializableIntegrationResponse>>,
    pub tls_config: Option<SerializableTlsConfig>,
}

impl From<Integration> for SerializableIntegration {
    fn from(value: Integration) -> Self {
        Self {
            r#type: value.r#type.map(SerializableIntegrationType::from),
            http_method: value.http_method,
            uri: value.uri,
            connection_type: value.connection_type.map(SerializableConnectionType::from),
            connection_id: value.connection_id,
            credentials: value.credentials,
            request_parameters: value.request_parameters,
            request_templates: value.request_templates,
            passthrough_behavior: value.passthrough_behavior,
            content_handling: value
                .content_handling
                .map(SerializableContentHandlingStrategy::from),
            timeout_in_millis: value.timeout_in_millis,
            cache_namespace: value.cache_namespace,
            cache_key_parameters: value.cache_key_parameters,
            integration_responses: value.integration_responses.map(|responses| {
                responses
                    .into_iter()
                    .map(|(key, value)| (key, SerializableIntegrationResponse::from(value)))
                    .collect()
            }),
            tls_config: value.tls_config.map(SerializableTlsConfig::from),
        }
    }
}

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct SerializableTlsConfig {
    pub insecure_skip_verification: bool,
}
impl From<TlsConfig> for SerializableTlsConfig {
    fn from(value: TlsConfig) -> Self {
        Self {
            insecure_skip_verification: value.insecure_skip_verification,
        }
    }
}

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct SerializableIntegrationResponse {
    pub status_code: Option<String>,
    pub selection_pattern: Option<String>,
    pub response_parameters: Option<HashMap<String, String>>,
    pub response_templates: Option<HashMap<String, String>>,
    pub content_handling: Option<SerializableContentHandlingStrategy>,
}

impl From<IntegrationResponse> for SerializableIntegrationResponse {
    fn from(value: IntegrationResponse) -> Self {
        Self {
            status_code: value.status_code,
            selection_pattern: value.selection_pattern,
            response_parameters: value.response_parameters,
            response_templates: value.response_templates,
            content_handling: value
                .content_handling
                .map(SerializableContentHandlingStrategy::from),
        }
    }
}

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub enum SerializableContentHandlingStrategy {
    #[default]
    ConvertToBinary,
    ConvertToText,
    Unknown,
}
impl From<ContentHandlingStrategy> for SerializableContentHandlingStrategy {
    fn from(value: ContentHandlingStrategy) -> Self {
        match value {
            ContentHandlingStrategy::ConvertToBinary => {
                SerializableContentHandlingStrategy::ConvertToBinary
            }
            ContentHandlingStrategy::ConvertToText => {
                SerializableContentHandlingStrategy::ConvertToText
            }
            _ => SerializableContentHandlingStrategy::Unknown,
        }
    }
}
#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub enum SerializableConnectionType {
    #[default]
    Internet,
    VpcLink,
    Unknown,
}
impl From<ConnectionType> for SerializableConnectionType {
    fn from(value: ConnectionType) -> Self {
        match value {
            ConnectionType::Internet => SerializableConnectionType::Internet,
            ConnectionType::VpcLink => SerializableConnectionType::VpcLink,
            _ => SerializableConnectionType::Unknown,
        }
    }
}

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub enum SerializableIntegrationType {
    #[default]
    Aws,
    AwsProxy,
    Http,
    HttpProxy,
    Mock,
    Unknown,
}
impl From<IntegrationType> for SerializableIntegrationType {
    fn from(value: IntegrationType) -> Self {
        match value {
            IntegrationType::Aws => SerializableIntegrationType::Aws,
            IntegrationType::AwsProxy => SerializableIntegrationType::AwsProxy,
            IntegrationType::Http => SerializableIntegrationType::Http,
            IntegrationType::HttpProxy => SerializableIntegrationType::HttpProxy,
            IntegrationType::Mock => SerializableIntegrationType::Mock,
            _ => SerializableIntegrationType::Unknown,
        }
    }
}

impl SerializableResourceInput {
    pub fn to_aws_input(self, client: &Client) -> anyhow::Result<CreateResourceFluentBuilder> {
        Ok(client
            .create_resource()
            .set_rest_api_id(self.rest_api_id)
            .set_parent_id(self.parent_id)
            .set_path_part(self.path_part))
    }
    pub fn to_aws_update(
        self,
        client: &Client,
        resource_id: Option<String>,
    ) -> anyhow::Result<UpdateResourceFluentBuilder> {
        Ok(client
            .update_resource()
            .set_rest_api_id(self.rest_api_id)
            .set_resource_id(resource_id)
            .set_patch_operations(Some(vec![PatchOperationBuilder::default()
                .op(Op::Replace)
                .path("/pathPart")
                .set_value(self.path_part)
                .build()])))
    }
}

impl From<aws_sdk_apigateway::types::Resource> for SerializableGetResourceOutput {
    fn from(api: aws_sdk_apigateway::types::Resource) -> Self {
        SerializableGetResourceOutput {
            rest_api_id: None,
            id: api.id,
            parent_id: api.parent_id,
            path_part: api.path_part,
            path: api.path,
            resource_methods: api.resource_methods.map(|methods| {
                methods
                    .into_iter()
                    .map(|(key, value)| (key, SerializableMethod::from(value)))
                    .collect()
            }),
        }
    }
}

impl From<GetResourceOutput> for SerializableGetResourceOutput {
    fn from(output: GetResourceOutput) -> Self {
        SerializableGetResourceOutput {
            rest_api_id: None,
            id: output.id,
            parent_id: output.parent_id,
            path_part: output.path_part,
            path: output.path,
            resource_methods: output.resource_methods.map(|methods| {
                methods
                    .into_iter()
                    .map(|(key, value)| (key, SerializableMethod::from(value)))
                    .collect()
            }),
        }
    }
}
