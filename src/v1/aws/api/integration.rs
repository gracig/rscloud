use std::collections::HashMap;

use aws_sdk_apigateway::{
    operation::{
        get_integration::GetIntegrationOutput,
        put_integration::builders::PutIntegrationFluentBuilder,
        update_integration::builders::UpdateIntegrationFluentBuilder,
    },
    Client,
};
use serde::{Deserialize, Serialize};

use crate::prelude::{
    AwsManager, SerializableConnectionType, SerializableContentHandlingStrategy,
    SerializableIntegrationResponse, SerializableIntegrationType, SerializableTlsConfig,
};
use crate::{
    prelude::{AwsResource, AwsResourceCreator},
    v1::manager::{ManagerError, ResourceManager},
};

pub type RestAPIIntegrationInput = SerializablePutIntegrationInput;
pub type RestAPIIntegrationOutput = SerializableGetIntegrationOutput;
pub type RestAPIIntegrationManager =
    AwsManager<RestAPIIntegrationInput, RestAPIIntegrationOutput, Client>;
pub type RestAPIIntegration<'a> =
    AwsResource<'a, RestAPIIntegrationInput, RestAPIIntegrationOutput>;

impl RestAPIIntegration<'_> {}

impl AwsResourceCreator for RestAPIIntegration<'_> {
    type Input = RestAPIIntegrationInput;
    type Output = RestAPIIntegrationOutput;
    fn r#type() -> crate::prelude::AwsType {
        crate::prelude::AwsType::RestAPIIntegration
    }
    fn manager(
        handle: &tokio::runtime::Handle,
        config: &aws_config::SdkConfig,
    ) -> std::sync::Arc<dyn ResourceManager<Self::Input, Self::Output>> {
        RestAPIIntegrationManager::new(handle, Client::new(config), config).arc()
    }

    fn input_hook(_id: &str, _input: &mut Self::Input) {}
}
impl RestAPIIntegrationManager {
    fn lookup(
        &self,
        rest_api_id: Option<String>,
        resource_id: Option<String>,
        http_method: Option<String>,
    ) -> Result<Option<RestAPIIntegrationOutput>, crate::v1::manager::ManagerError> {
        self.handle.block_on(async {
            self.client
                .get_integration()
                .set_rest_api_id(rest_api_id.clone())
                .set_resource_id(resource_id.clone())
                .set_http_method(http_method)
                .send()
                .await
                .map_err(|e| ManagerError::LookupFail(format!("{:?}", e.into_source())))
                .map(RestAPIIntegrationOutput::from)
                .map(|mut r| {
                    r.rest_api_id = rest_api_id;
                    r.resource_id = resource_id;
                    r
                })
                .map(Some)
                .or_else(|e| match e {
                    ManagerError::LookupFail(ref msg) if msg.contains("NotFound") => Ok(None),
                    _ => Err(e),
                })
        })
    }

    fn create(
        &self,
        input: &mut RestAPIIntegrationInput,
    ) -> Result<RestAPIIntegrationOutput, crate::v1::manager::ManagerError> {
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
        latest: &RestAPIIntegrationOutput,
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
        _latest: &RestAPIIntegrationOutput,
        input: &mut RestAPIIntegrationInput,
    ) -> Result<Option<RestAPIIntegrationOutput>, crate::v1::manager::ManagerError> {
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

impl ResourceManager<RestAPIIntegrationInput, RestAPIIntegrationOutput>
    for RestAPIIntegrationManager
{
    fn lookup(
        &self,
        latest: &RestAPIIntegrationOutput,
    ) -> Result<Option<RestAPIIntegrationOutput>, ManagerError> {
        self.lookup(
            latest.rest_api_id.clone(),
            latest.resource_id.clone(),
            latest.http_method.clone(),
        )
    }

    fn lookup_by_input(
        &self,
        input: &RestAPIIntegrationInput,
    ) -> Result<Option<RestAPIIntegrationOutput>, ManagerError> {
        self.lookup(
            input.rest_api_id.clone(),
            input.resource_id.clone(),
            input.http_method.clone(),
        )
    }

    fn create(
        &self,
        input: &mut RestAPIIntegrationInput,
    ) -> Result<RestAPIIntegrationOutput, ManagerError> {
        self.create(input)
    }

    fn delete(&self, latest: &RestAPIIntegrationOutput) -> Result<bool, ManagerError> {
        self.delete(latest)
    }

    fn syncup(
        &self,
        latest: &RestAPIIntegrationOutput,
        input: &mut RestAPIIntegrationInput,
    ) -> Result<Option<RestAPIIntegrationOutput>, ManagerError> {
        self.syncup(latest, input)
    }
}

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct SerializablePutIntegrationInput {
    pub rest_api_id: Option<String>,
    pub resource_id: Option<String>,
    pub http_method: Option<String>,
    pub r#type: Option<SerializableIntegrationType>,
    pub integration_http_method: Option<String>,
    pub uri: Option<String>,
    pub connection_type: Option<SerializableConnectionType>,
    pub connection_id: Option<String>,
    pub credentials: Option<String>,
    pub request_parameters: Option<HashMap<String, String>>,
    pub request_templates: Option<HashMap<String, String>>,
    pub passthrough_behavior: Option<String>,
    pub cache_namespace: Option<String>,
    pub cache_key_parameters: Option<Vec<String>>,
    pub content_handling: Option<SerializableContentHandlingStrategy>,
    pub timeout_in_millis: Option<i32>,
    pub tls_config: Option<SerializableTlsConfig>,
}

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct SerializableGetIntegrationOutput {
    pub resource_id: Option<String>,
    pub rest_api_id: Option<String>,
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

impl SerializablePutIntegrationInput {
    pub fn to_aws_input(self, client: &Client) -> anyhow::Result<PutIntegrationFluentBuilder> {
        Ok(client
            .put_integration()
            .set_rest_api_id(self.rest_api_id)
            .set_resource_id(self.resource_id)
            .set_http_method(self.http_method)
            .set_integration_http_method(self.integration_http_method)
            .set_uri(self.uri)
            .set_type(self.r#type.map(|t| t.into()))
            .set_connection_type(self.connection_type.map(|t| t.into()))
            .set_connection_id(self.connection_id)
            .set_credentials(self.credentials)
            .set_request_parameters(self.request_parameters)
            .set_request_templates(self.request_templates)
            .set_passthrough_behavior(self.passthrough_behavior)
            .set_cache_namespace(self.cache_namespace)
            .set_cache_key_parameters(self.cache_key_parameters)
            .set_content_handling(self.content_handling.map(|t| t.into()))
            .set_timeout_in_millis(self.timeout_in_millis)
            .set_tls_config(self.tls_config.map(|t| t.into())))
    }
    pub fn to_aws_update(self, client: &Client) -> anyhow::Result<UpdateIntegrationFluentBuilder> {
        Ok(client
            .update_integration()
            .set_rest_api_id(self.rest_api_id)
            .set_resource_id(self.resource_id)
            .set_http_method(self.http_method)
            .set_patch_operations(Some(vec![])))
    }
}

impl From<GetIntegrationOutput> for SerializableGetIntegrationOutput {
    fn from(output: GetIntegrationOutput) -> Self {
        Self {
            resource_id: None,
            rest_api_id: None,
            r#type: output.r#type.map(SerializableIntegrationType::from),
            http_method: output.http_method,
            uri: output.uri,
            connection_type: output.connection_type.map(SerializableConnectionType::from),
            connection_id: output.connection_id,
            credentials: output.credentials,
            request_parameters: output.request_parameters,
            request_templates: output.request_templates,
            passthrough_behavior: output.passthrough_behavior,
            content_handling: output
                .content_handling
                .map(SerializableContentHandlingStrategy::from),
            timeout_in_millis: output.timeout_in_millis,
            cache_namespace: output.cache_namespace,
            cache_key_parameters: output.cache_key_parameters,
            integration_responses: output.integration_responses.map(|responses| {
                responses
                    .into_iter()
                    .map(|(key, value)| (key, SerializableIntegrationResponse::from(value)))
                    .collect()
            }),
            tls_config: output.tls_config.map(SerializableTlsConfig::from),
        }
    }
}
