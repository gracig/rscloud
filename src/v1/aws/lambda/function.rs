use anyhow::Context;
use aws_sdk_lambda::operation::create_function::builders::CreateFunctionFluentBuilder;
use aws_sdk_lambda::operation::get_function::GetFunctionOutput;
use aws_sdk_lambda::primitives::Blob;
use aws_sdk_lambda::types::builders::{
    DeadLetterConfigBuilder, EnvironmentBuilder, EphemeralStorageBuilder, FileSystemConfigBuilder,
    FunctionCodeBuilder, ImageConfigBuilder, LoggingConfigBuilder, SnapStartBuilder,
    TracingConfigBuilder, VpcConfigBuilder,
};
use aws_sdk_lambda::types::{
    ApplicationLogLevel, Architecture, Concurrency, DeadLetterConfig, Environment,
    EnvironmentResponse, EphemeralStorage, FileSystemConfig, FunctionCode, FunctionCodeLocation,
    FunctionConfiguration, ImageConfig, LastUpdateStatus, LastUpdateStatusReasonCode, Layer,
    LogFormat, LoggingConfig, PackageType, Runtime, RuntimeVersionConfig, RuntimeVersionError,
    SnapStart, SnapStartApplyOn, SnapStartResponse, State, StateReasonCode, SystemLogLevel,
    TracingConfig, TracingConfigResponse, TracingMode, VpcConfig, VpcConfigResponse,
};
use aws_sdk_lambda::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::prelude::{
    ArnProvider, AwsManager, AwsResource, AwsResourceCreator, ResourceError, Role,
};
use crate::v1::manager::{ManagerError, ResourceManager};

pub type FunctionInput = SerializableCreateFunctionInput;
pub type FunctionOutput = SerializableFunctionOutput;
pub type FunctionManager = AwsManager<FunctionInput, FunctionOutput, Client>;
pub type Function<'a> = AwsResource<'a, FunctionInput, FunctionOutput>;

impl AwsResourceCreator for Function<'_> {
    type Input = FunctionInput;
    type Output = FunctionOutput;
    fn r#type() -> crate::prelude::AwsType {
        crate::prelude::AwsType::Function
    }
    fn manager(
        handle: &tokio::runtime::Handle,
        config: &aws_config::SdkConfig,
    ) -> std::sync::Arc<dyn ResourceManager<Self::Input, Self::Output>> {
        FunctionManager::new(handle, Client::new(config), config).arc()
    }
    fn input_hook(id: &str, input: &mut Self::Input) {
        if input.function_name.is_none() {
            input.function_name = Some(id.to_string())
        }
    }
}
impl ArnProvider for FunctionOutput {
    fn arn(&self) -> Option<Vec<String>> {
        self.configuration
            .as_ref()
            .and_then(|config| config.function_arn.clone().map(|arn| vec![arn]))
    }
}
impl<'a> Function<'a> {
    pub fn bind_role(&self, role: &Role) -> Result<(), ResourceError> {
        self.bind(role, move |this, other| this.role = Some(other.arn.clone()))
    }
}
impl FunctionManager {
    fn lookup(&self, function_name: &str) -> Result<Option<FunctionOutput>, ManagerError> {
        self.handle.block_on(async {
            self.client
                .get_function()
                .function_name(function_name)
                .send()
                .await
                .map(SerializableFunctionOutput::from)
                .map(Some)
                .map_err(|e| ManagerError::LookupFail(format!("{:?}", e.into_source())))
                .or_else(|e| match e {
                    ManagerError::LookupFail(ref msg) if msg.contains("ResourceNotFound") => {
                        Ok(None)
                    }
                    _ => Err(e),
                })
        })
    }
    fn create(&self, input: &mut FunctionInput) -> Result<FunctionOutput, ManagerError> {
        let output = self.handle.block_on(async {
            input
                .clone()
                .to_aws_input(&self.client)
                .map_err(|e| ManagerError::CreateFail(format!("{:?}", e)))?
                .send()
                .await
                .map_err(|e| ManagerError::CreateFail(format!("{:?}", e.into_source())))
        })?;
        if let Some(function_name) = output.function_name() {
            match self.lookup(function_name)? {
                Some(f) => Ok(f),
                None => Err(ManagerError::CreateFail(
                    "Function was created but not found in the cloud.".to_string(),
                )),
            }
        } else {
            Err(ManagerError::CreateFail(
                "Function created but not found.".to_string(),
            ))
        }
    }

    fn delete(&self, function_name: &str) -> Result<bool, crate::v1::manager::ManagerError> {
        self.handle.block_on(async {
            self.client
                .delete_function()
                .function_name(function_name)
                .send()
                .await
                .map(|_| true)
                .map_err(|e| ManagerError::DeleteFail(format!("{:?}", e.into_source())))
        })
    }
    fn syncup(
        &self,
        _latest: &FunctionOutput,
        input: &mut FunctionInput,
    ) -> Result<Option<FunctionOutput>, crate::v1::manager::ManagerError> {
        self.handle.block_on(async {
            match input.clone().to_aws_input(&self.client) {
                Ok(input) => self
                    .client
                    .update_function_configuration()
                    .set_function_name(input.get_function_name().clone())
                    .set_environment(input.get_environment().clone())
                    .set_description(input.get_description().clone())
                    .set_timeout(*input.get_timeout())
                    .set_memory_size(*input.get_memory_size())
                    .send()
                    .await
                    .map_err(|e| ManagerError::UpdateFail(format!("{:?}", e.into_source()))),
                Err(e) => Err(ManagerError::UpdateFail(format!("{:?}", e))),
            }
        })?;
        self.handle.block_on(async {
            match input.clone().to_aws_input(&self.client) {
                Ok(input) => self
                    .client
                    .update_function_code()
                    .set_function_name(input.get_function_name().clone())
                    .set_zip_file(input.get_code().clone().unwrap().zip_file)
                    .send()
                    .await
                    .map_err(|e| ManagerError::UpdateFail(format!("{:?}", e.into_source()))),
                Err(e) => Err(ManagerError::UpdateFail(format!("{:?}", e))),
            }
        })?;
        if let Some(function_name) = input.function_name.as_ref() {
            match self.lookup(function_name)? {
                Some(f) => Ok(Some(f)),
                None => Err(ManagerError::CreateFail(
                    "Function was created but not found in the cloud.".to_string(),
                )),
            }
        } else {
            Err(ManagerError::UpdateFail(
                "Function updated but not found.".to_string(),
            ))
        }
    }
}

impl ResourceManager<FunctionInput, FunctionOutput> for FunctionManager {
    fn lookup(
        &self,
        latest: &FunctionOutput,
    ) -> Result<Option<FunctionOutput>, crate::v1::manager::ManagerError> {
        self.lookup(
            latest
                .configuration
                .as_ref()
                .unwrap()
                .function_name
                .as_ref()
                .unwrap(),
        )
    }

    fn create(
        &self,
        input: &mut FunctionInput,
    ) -> Result<FunctionOutput, crate::v1::manager::ManagerError> {
        self.create(input)
    }

    fn delete(&self, latest: &FunctionOutput) -> Result<bool, crate::v1::manager::ManagerError> {
        self.delete(
            latest
                .configuration
                .as_ref()
                .unwrap()
                .function_name
                .as_ref()
                .unwrap(),
        )
    }

    fn syncup(
        &self,
        latest: &FunctionOutput,
        input: &mut FunctionInput,
    ) -> Result<Option<FunctionOutput>, crate::v1::manager::ManagerError> {
        self.syncup(latest, input)
    }

    fn lookup_by_input(
        &self,
        input: &FunctionInput,
    ) -> Result<Option<FunctionOutput>, ManagerError> {
        if let Some(function_name) = &input.function_name {
            self.lookup(function_name)
        } else {
            Err(ManagerError::CreateFail(
                "Function name is required.".to_string(),
            ))
        }
    }
}

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct SerializableFunctionOutput {
    pub configuration: Option<SerializableFunctionConfiguration>,
    pub code: Option<SerializableFunctionCodeLocation>,
    pub tags: Option<HashMap<String, String>>,
    pub concurrency: Option<SerializableConcurrency>,
}

impl From<GetFunctionOutput> for SerializableFunctionOutput {
    fn from(output: GetFunctionOutput) -> Self {
        Self {
            configuration: output
                .configuration
                .map(SerializableFunctionConfiguration::from),
            code: output.code.map(SerializableFunctionCodeLocation::from),
            tags: output.tags,
            concurrency: output.concurrency.map(SerializableConcurrency::from),
        }
    }
}

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct SerializableConcurrency {
    pub reserved_concurrent_executions: Option<i32>,
}

impl From<Concurrency> for SerializableConcurrency {
    fn from(concurrency: Concurrency) -> Self {
        Self {
            reserved_concurrent_executions: concurrency.reserved_concurrent_executions,
        }
    }
}
#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct SerializableFunctionCodeLocation {
    pub repository_type: Option<String>,
    pub location: Option<String>,
    pub image_uri: Option<String>,
    pub resolved_image_uri: Option<String>,
}
impl From<FunctionCodeLocation> for SerializableFunctionCodeLocation {
    fn from(code_location: FunctionCodeLocation) -> Self {
        Self {
            repository_type: code_location.repository_type,
            location: code_location.location,
            image_uri: code_location.image_uri,
            resolved_image_uri: code_location.resolved_image_uri,
        }
    }
}

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct SerializableFunctionConfiguration {
    pub function_name: Option<String>,
    pub function_arn: Option<String>,
    pub runtime: Option<SerializableRuntime>,
    pub role: Option<String>,
    pub handler: Option<String>,
    pub code_size: i64,
    pub description: Option<String>,
    pub timeout: Option<i32>,
    pub memory_size: Option<i32>,
    pub last_modified: Option<String>,
    pub code_sha256: Option<String>,
    pub version: Option<String>,
    pub vpc_config: Option<SerializableVpcConfig>,
    pub dead_letter_config: Option<SerializableDeadLetterConfig>,
    pub environment: Option<SerializableEnvironment>,
    pub kms_key_arn: Option<String>,
    pub tracing_config: Option<SerializableTracingConfig>,
    pub master_arn: Option<String>,
    pub revision_id: Option<String>,
    pub layers: Option<Vec<SerializableLayer>>,
    pub state: Option<SerializableState>,
    pub state_reason: Option<String>,
    pub state_reason_code: Option<SerializableStateReasonCode>,
    pub last_update_status: Option<SerializableLastUpdateStatus>,
    pub last_update_status_reason: Option<String>,
    pub last_update_status_reason_code: Option<SerializableLastUpdateStatusReasonCode>,
    pub file_system_configs: Option<Vec<SerializableFileSystemConfig>>,
    pub package_type: Option<SerializablePackageType>,
    pub image_config: Option<SerializableImageConfig>,
    pub signing_profile_version_arn: Option<String>,
    pub signing_job_arn: Option<String>,
    pub architectures: Option<Vec<SerializableArchitecture>>,
    pub ephemeral_storage: Option<SerializableEphemeralStorage>,
    pub snap_start: Option<SerializableSnapStart>,
    pub runtime_version_config: Option<SerializableRuntimeVersionConfig>,
    pub logging_config: Option<SerializableLoggingConfig>,
}

impl From<FunctionConfiguration> for SerializableFunctionConfiguration {
    fn from(function_configuration: FunctionConfiguration) -> Self {
        Self {
            function_name: function_configuration.function_name,
            function_arn: function_configuration.function_arn,
            runtime: function_configuration
                .runtime
                .map(SerializableRuntime::from),
            role: function_configuration.role,
            handler: function_configuration.handler,
            code_size: function_configuration.code_size,
            description: function_configuration.description,
            timeout: function_configuration.timeout,
            memory_size: function_configuration.memory_size,
            last_modified: function_configuration.last_modified,
            code_sha256: function_configuration.code_sha256,
            version: function_configuration.version,
            vpc_config: function_configuration
                .vpc_config
                .map(SerializableVpcConfig::from),
            dead_letter_config: function_configuration
                .dead_letter_config
                .map(SerializableDeadLetterConfig::from),
            environment: function_configuration
                .environment
                .map(SerializableEnvironment::from),
            kms_key_arn: function_configuration.kms_key_arn,
            tracing_config: function_configuration
                .tracing_config
                .map(SerializableTracingConfig::from),
            master_arn: function_configuration.master_arn,
            revision_id: function_configuration.revision_id,
            layers: function_configuration
                .layers
                .map(|l| l.into_iter().map(SerializableLayer::from).collect()),
            state: function_configuration.state.map(SerializableState::from),
            state_reason: function_configuration.state_reason,
            state_reason_code: function_configuration
                .state_reason_code
                .map(SerializableStateReasonCode::from),
            last_update_status: function_configuration
                .last_update_status
                .map(SerializableLastUpdateStatus::from),
            last_update_status_reason: function_configuration.last_update_status_reason,
            last_update_status_reason_code: function_configuration
                .last_update_status_reason_code
                .map(SerializableLastUpdateStatusReasonCode::from),
            file_system_configs: function_configuration.file_system_configs.map(|v| {
                v.into_iter()
                    .map(SerializableFileSystemConfig::from)
                    .collect()
            }),
            package_type: function_configuration
                .package_type
                .map(SerializablePackageType::from),
            image_config: function_configuration
                .image_config_response
                .and_then(|img| img.image_config)
                .map(SerializableImageConfig::from),
            signing_profile_version_arn: function_configuration.signing_profile_version_arn,
            signing_job_arn: function_configuration.signing_job_arn,
            architectures: function_configuration
                .architectures
                .map(|v| v.into_iter().map(SerializableArchitecture::from).collect()),
            ephemeral_storage: function_configuration
                .ephemeral_storage
                .map(SerializableEphemeralStorage::from),
            snap_start: function_configuration
                .snap_start
                .map(SerializableSnapStart::from),
            runtime_version_config: function_configuration
                .runtime_version_config
                .map(SerializableRuntimeVersionConfig::from),
            logging_config: function_configuration
                .logging_config
                .map(SerializableLoggingConfig::from),
        }
    }
}

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct SerializableRuntimeVersionConfig {
    pub runtime_version_arn: Option<String>,
    pub error: Option<SerializableRuntimeVersionError>,
}
impl From<RuntimeVersionConfig> for SerializableRuntimeVersionConfig {
    fn from(runtime_version_config: RuntimeVersionConfig) -> Self {
        Self {
            runtime_version_arn: runtime_version_config.runtime_version_arn,
            error: runtime_version_config
                .error
                .map(SerializableRuntimeVersionError::from),
        }
    }
}

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct SerializableRuntimeVersionError {
    pub error_code: Option<String>,
    pub message: Option<String>,
}

impl From<RuntimeVersionError> for SerializableRuntimeVersionError {
    fn from(runtime_version_error: RuntimeVersionError) -> Self {
        Self {
            error_code: runtime_version_error.error_code,
            message: runtime_version_error.message,
        }
    }
}

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub enum SerializableLastUpdateStatusReasonCode {
    #[default]
    DisabledKmsKey,
    EfsioError,
    EfsMountConnectivityError,
    EfsMountFailure,
    EfsMountTimeout,
    EniLimitExceeded,
    FunctionError,
    ImageAccessDenied,
    ImageDeleted,
    InsufficientRolePermissions,
    InternalError,
    InvalidConfiguration,
    InvalidImage,
    InvalidRuntime,
    InvalidSecurityGroup,
    InvalidStateKmsKey,
    InvalidSubnet,
    InvalidZipFileException,
    KmsKeyAccessDenied,
    KmsKeyNotFound,
    SubnetOutOfIpAddresses,
    Unknown,
}
impl From<LastUpdateStatusReasonCode> for SerializableLastUpdateStatusReasonCode {
    fn from(last_update_status_reason_code: LastUpdateStatusReasonCode) -> Self {
        match last_update_status_reason_code {
            LastUpdateStatusReasonCode::DisabledKmsKey => Self::DisabledKmsKey,
            LastUpdateStatusReasonCode::EfsioError => Self::EfsioError,
            LastUpdateStatusReasonCode::EfsMountConnectivityError => {
                Self::EfsMountConnectivityError
            }
            LastUpdateStatusReasonCode::EfsMountFailure => Self::EfsMountFailure,
            LastUpdateStatusReasonCode::EfsMountTimeout => Self::EfsMountTimeout,
            LastUpdateStatusReasonCode::EniLimitExceeded => Self::EniLimitExceeded,
            LastUpdateStatusReasonCode::FunctionError => Self::FunctionError,
            LastUpdateStatusReasonCode::ImageAccessDenied => Self::ImageAccessDenied,
            LastUpdateStatusReasonCode::ImageDeleted => Self::ImageDeleted,
            LastUpdateStatusReasonCode::InsufficientRolePermissions => {
                Self::InsufficientRolePermissions
            }
            LastUpdateStatusReasonCode::InternalError => Self::InternalError,
            LastUpdateStatusReasonCode::InvalidConfiguration => Self::InvalidConfiguration,
            LastUpdateStatusReasonCode::InvalidImage => Self::InvalidImage,
            LastUpdateStatusReasonCode::InvalidRuntime => Self::InvalidRuntime,
            LastUpdateStatusReasonCode::InvalidSecurityGroup => Self::InvalidSecurityGroup,
            LastUpdateStatusReasonCode::InvalidStateKmsKey => Self::InvalidStateKmsKey,
            LastUpdateStatusReasonCode::InvalidSubnet => Self::InvalidSubnet,
            LastUpdateStatusReasonCode::InvalidZipFileException => Self::InvalidZipFileException,
            LastUpdateStatusReasonCode::KmsKeyAccessDenied => Self::KmsKeyAccessDenied,
            LastUpdateStatusReasonCode::KmsKeyNotFound => Self::KmsKeyNotFound,
            LastUpdateStatusReasonCode::SubnetOutOfIpAddresses => Self::SubnetOutOfIpAddresses,
            _ => Self::Unknown,
        }
    }
}

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub enum SerializableLastUpdateStatus {
    Failed,
    #[default]
    InProgress,
    Successful,
    Unknown,
}

impl From<LastUpdateStatus> for SerializableLastUpdateStatus {
    fn from(last_update_status: LastUpdateStatus) -> Self {
        match last_update_status {
            LastUpdateStatus::Failed => Self::Failed,
            LastUpdateStatus::InProgress => Self::InProgress,
            LastUpdateStatus::Successful => Self::Successful,
            _ => Self::Unknown,
        }
    }
}

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub enum SerializableStateReasonCode {
    Creating,
    DisabledKmsKey,
    EfsioError,
    EfsMountConnectivityError,
    EfsMountFailure,
    EfsMountTimeout,
    EniLimitExceeded,
    FunctionError,
    #[default]
    Idle,
    ImageAccessDenied,
    ImageDeleted,
    InsufficientRolePermissions,
    InternalError,
    InvalidConfiguration,
    InvalidImage,
    InvalidRuntime,
    InvalidSecurityGroup,
    InvalidStateKmsKey,
    InvalidSubnet,
    InvalidZipFileException,
    KmsKeyAccessDenied,
    KmsKeyNotFound,
    Restoring,
    SubnetOutOfIpAddresses,
    Unknown,
}

impl From<StateReasonCode> for SerializableStateReasonCode {
    fn from(state_reason_code: StateReasonCode) -> Self {
        match state_reason_code {
            StateReasonCode::Creating => Self::Creating,
            StateReasonCode::DisabledKmsKey => Self::DisabledKmsKey,
            StateReasonCode::EfsioError => Self::EfsioError,
            StateReasonCode::EfsMountConnectivityError => Self::EfsMountConnectivityError,
            StateReasonCode::EfsMountFailure => Self::EfsMountFailure,
            StateReasonCode::EfsMountTimeout => Self::EfsMountTimeout,
            StateReasonCode::EniLimitExceeded => Self::EniLimitExceeded,
            StateReasonCode::FunctionError => Self::FunctionError,
            StateReasonCode::Idle => Self::Idle,
            StateReasonCode::ImageAccessDenied => Self::ImageAccessDenied,
            StateReasonCode::ImageDeleted => Self::ImageDeleted,
            StateReasonCode::InsufficientRolePermissions => Self::InsufficientRolePermissions,
            StateReasonCode::InternalError => Self::InternalError,
            StateReasonCode::InvalidConfiguration => Self::InvalidConfiguration,
            StateReasonCode::InvalidImage => Self::InvalidImage,
            StateReasonCode::InvalidRuntime => Self::InvalidRuntime,
            StateReasonCode::InvalidSecurityGroup => Self::InvalidSecurityGroup,
            StateReasonCode::InvalidStateKmsKey => Self::InvalidStateKmsKey,
            StateReasonCode::InvalidSubnet => Self::InvalidSubnet,
            StateReasonCode::InvalidZipFileException => Self::InvalidZipFileException,
            StateReasonCode::KmsKeyAccessDenied => Self::KmsKeyAccessDenied,
            StateReasonCode::KmsKeyNotFound => Self::KmsKeyNotFound,
            StateReasonCode::Restoring => Self::Restoring,
            StateReasonCode::SubnetOutOfIpAddresses => Self::SubnetOutOfIpAddresses,
            _ => Self::Unknown,
        }
    }
}

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub enum SerializableState {
    Active,
    Failed,
    Inactive,
    #[default]
    Pending,
    Unknown,
}
impl From<State> for SerializableState {
    fn from(state: State) -> Self {
        match state {
            State::Active => Self::Active,
            State::Failed => Self::Failed,
            State::Inactive => Self::Inactive,
            State::Pending => Self::Pending,
            _ => Self::Unknown,
        }
    }
}

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct SerializableLayer {
    pub arn: Option<String>,
    pub code_size: i64,
    pub signing_profile_version_arn: Option<String>,
    pub signing_job_arn: Option<String>,
}

impl From<Layer> for SerializableLayer {
    fn from(layer: Layer) -> Self {
        Self {
            arn: layer.arn,
            code_size: layer.code_size,
            signing_profile_version_arn: layer.signing_profile_version_arn,
            signing_job_arn: layer.signing_job_arn,
        }
    }
}

impl SerializableCreateFunctionInput {
    pub fn to_aws_input(self, client: &Client) -> anyhow::Result<CreateFunctionFluentBuilder> {
        let client = client
            .create_function()
            .set_architectures(
                self.architectures
                    .map(|v| v.into_iter().map(SerializableArchitecture::into).collect()),
            )
            .set_code(self.code.map(SerializableFunctionCode::into))
            .set_code_signing_config_arn(self.code_signing_config_arn)
            .set_dead_letter_config(
                self.dead_letter_config
                    .map(SerializableDeadLetterConfig::into),
            )
            .set_description(self.description)
            .set_environment(self.environment.map(SerializableEnvironment::into))
            .set_ephemeral_storage(if let Some(e) = self.ephemeral_storage {
                Some(e.try_into()?)
            } else {
                None
            })
            .set_function_name(self.function_name)
            .set_handler(self.handler)
            .set_image_config(self.image_config.map(SerializableImageConfig::into))
            .set_kms_key_arn(self.kms_key_arn)
            .set_layers(self.layers)
            .set_logging_config(self.logging_config.map(SerializableLoggingConfig::into))
            .set_memory_size(self.memory_size)
            .set_package_type(self.package_type.map(SerializablePackageType::into))
            .set_publish(self.publish)
            .set_role(self.role)
            .set_runtime(self.runtime.map(SerializableRuntime::into))
            .set_snap_start(self.snap_start.map(SerializableSnapStart::into))
            .set_tags(self.tags)
            .set_timeout(self.timeout)
            .set_tracing_config(self.tracing_config.map(SerializableTracingConfig::into))
            .set_file_system_configs(if let Some(v) = self.file_system_configs {
                let mut vs = vec![];
                for f in v {
                    vs.push(f.try_into()?);
                }
                Some(vs)
            } else {
                None
            })
            .set_vpc_config(self.vpc_config.map(SerializableVpcConfig::into));

        Ok(client)
    }
}

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct SerializableCreateFunctionInput {
    pub function_name: Option<String>,
    pub runtime: Option<SerializableRuntime>,
    pub role: Option<String>,
    pub handler: Option<String>,
    pub code: Option<SerializableFunctionCode>,
    pub description: Option<String>,
    pub timeout: Option<i32>,
    pub memory_size: Option<i32>,
    pub publish: Option<bool>,
    pub vpc_config: Option<SerializableVpcConfig>,
    pub package_type: Option<SerializablePackageType>,
    pub dead_letter_config: Option<SerializableDeadLetterConfig>,
    pub environment: Option<SerializableEnvironment>,
    pub kms_key_arn: Option<String>,
    pub tracing_config: Option<SerializableTracingConfig>,
    pub tags: Option<HashMap<String, String>>,
    pub layers: Option<Vec<String>>,
    pub file_system_configs: Option<Vec<SerializableFileSystemConfig>>,
    pub image_config: Option<SerializableImageConfig>,
    pub code_signing_config_arn: Option<String>,
    pub architectures: Option<Vec<SerializableArchitecture>>,
    pub ephemeral_storage: Option<SerializableEphemeralStorage>,
    pub snap_start: Option<SerializableSnapStart>,
    pub logging_config: Option<SerializableLoggingConfig>,
}

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct SerializableLoggingConfig {
    pub log_format: Option<SerializableLogFormat>,
    pub application_log_level: Option<SerializableApplicationLogLevel>,
    pub system_log_level: Option<SerializableSystemLogLevel>,
    pub log_group: Option<String>,
}
impl From<SerializableLoggingConfig> for LoggingConfig {
    fn from(value: SerializableLoggingConfig) -> Self {
        LoggingConfigBuilder::default()
            .set_log_group(value.log_group)
            .set_log_format(value.log_format.map(SerializableLogFormat::into))
            .set_application_log_level(
                value
                    .application_log_level
                    .map(SerializableApplicationLogLevel::into),
            )
            .set_system_log_level(value.system_log_level.map(SerializableSystemLogLevel::into))
            .build()
    }
}

impl From<LoggingConfig> for SerializableLoggingConfig {
    fn from(value: LoggingConfig) -> Self {
        SerializableLoggingConfig {
            log_format: value.log_format.map(SerializableLogFormat::from),
            application_log_level: value
                .application_log_level
                .map(SerializableApplicationLogLevel::from),
            system_log_level: value.system_log_level.map(SerializableSystemLogLevel::from),
            log_group: value.log_group,
        }
    }
}

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub enum SerializableSystemLogLevel {
    Debug,
    #[default]
    Info,
    Warn,
    Unknown,
}
impl From<SerializableSystemLogLevel> for SystemLogLevel {
    fn from(value: SerializableSystemLogLevel) -> Self {
        match value {
            SerializableSystemLogLevel::Debug => SystemLogLevel::Debug,
            SerializableSystemLogLevel::Info => SystemLogLevel::Info,
            SerializableSystemLogLevel::Warn => SystemLogLevel::Warn,
            _ => unimplemented!(),
        }
    }
}

impl From<SystemLogLevel> for SerializableSystemLogLevel {
    fn from(value: SystemLogLevel) -> Self {
        match value {
            SystemLogLevel::Debug => SerializableSystemLogLevel::Debug,
            SystemLogLevel::Info => SerializableSystemLogLevel::Info,
            SystemLogLevel::Warn => SerializableSystemLogLevel::Warn,
            _ => SerializableSystemLogLevel::Unknown,
        }
    }
}

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub enum SerializableApplicationLogLevel {
    Debug,
    #[default]
    Error,
    Fatal,
    Info,
    Trace,
    Warn,
    Unknown,
}

impl From<SerializableApplicationLogLevel> for ApplicationLogLevel {
    fn from(value: SerializableApplicationLogLevel) -> Self {
        match value {
            SerializableApplicationLogLevel::Debug => ApplicationLogLevel::Debug,
            SerializableApplicationLogLevel::Error => ApplicationLogLevel::Error,
            SerializableApplicationLogLevel::Fatal => ApplicationLogLevel::Fatal,
            SerializableApplicationLogLevel::Info => ApplicationLogLevel::Info,
            SerializableApplicationLogLevel::Trace => ApplicationLogLevel::Trace,
            SerializableApplicationLogLevel::Warn => ApplicationLogLevel::Warn,
            _ => unimplemented!(),
        }
    }
}

impl From<ApplicationLogLevel> for SerializableApplicationLogLevel {
    fn from(value: ApplicationLogLevel) -> Self {
        match value {
            ApplicationLogLevel::Debug => SerializableApplicationLogLevel::Debug,
            ApplicationLogLevel::Error => SerializableApplicationLogLevel::Error,
            ApplicationLogLevel::Fatal => SerializableApplicationLogLevel::Fatal,
            ApplicationLogLevel::Info => SerializableApplicationLogLevel::Info,
            ApplicationLogLevel::Trace => SerializableApplicationLogLevel::Trace,
            ApplicationLogLevel::Warn => SerializableApplicationLogLevel::Warn,
            _ => SerializableApplicationLogLevel::Unknown,
        }
    }
}

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub enum SerializableLogFormat {
    #[default]
    Json,
    Text,
    Unknown,
}
impl From<SerializableLogFormat> for LogFormat {
    fn from(value: SerializableLogFormat) -> Self {
        match value {
            SerializableLogFormat::Json => LogFormat::Json,
            SerializableLogFormat::Text => LogFormat::Text,
            _ => unimplemented!(),
        }
    }
}
impl From<LogFormat> for SerializableLogFormat {
    fn from(value: LogFormat) -> Self {
        match value {
            LogFormat::Json => SerializableLogFormat::Json,
            LogFormat::Text => SerializableLogFormat::Text,
            _ => SerializableLogFormat::Unknown,
        }
    }
}

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct SerializableSnapStart {
    pub apply_on: Option<SerializableSnapStartApplyOn>,
}

impl From<SerializableSnapStart> for SnapStart {
    fn from(value: SerializableSnapStart) -> Self {
        SnapStartBuilder::default()
            .set_apply_on(value.apply_on.map(SerializableSnapStartApplyOn::into))
            .build()
    }
}

impl From<SnapStart> for SerializableSnapStart {
    fn from(value: SnapStart) -> Self {
        SerializableSnapStart {
            apply_on: value.apply_on.map(SerializableSnapStartApplyOn::from),
        }
    }
}

impl From<SnapStartResponse> for SerializableSnapStart {
    fn from(value: SnapStartResponse) -> Self {
        SerializableSnapStart {
            apply_on: value.apply_on.map(SerializableSnapStartApplyOn::from),
        }
    }
}

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub enum SerializableSnapStartApplyOn {
    #[default]
    None,
    PublishedVersions,
    Unknown,
}

impl From<SerializableSnapStartApplyOn> for SnapStartApplyOn {
    fn from(value: SerializableSnapStartApplyOn) -> Self {
        match value {
            SerializableSnapStartApplyOn::None => SnapStartApplyOn::None,
            SerializableSnapStartApplyOn::PublishedVersions => SnapStartApplyOn::PublishedVersions,
            _ => unimplemented!(),
        }
    }
}
impl From<SnapStartApplyOn> for SerializableSnapStartApplyOn {
    fn from(value: SnapStartApplyOn) -> Self {
        match value {
            SnapStartApplyOn::None => SerializableSnapStartApplyOn::None,
            SnapStartApplyOn::PublishedVersions => SerializableSnapStartApplyOn::PublishedVersions,
            _ => SerializableSnapStartApplyOn::Unknown,
        }
    }
}

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct SerializableEphemeralStorage {
    pub size: i32,
}
impl TryFrom<SerializableEphemeralStorage> for EphemeralStorage {
    type Error = anyhow::Error;

    fn try_from(value: SerializableEphemeralStorage) -> anyhow::Result<Self, Self::Error> {
        EphemeralStorageBuilder::default()
            .set_size(Some(value.size))
            .build()
            .context("Error from Ephemeral Storage")
    }
}
impl From<EphemeralStorage> for SerializableEphemeralStorage {
    fn from(value: EphemeralStorage) -> Self {
        SerializableEphemeralStorage { size: value.size }
    }
}

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub enum SerializableArchitecture {
    Arm64,
    #[default]
    X8664,
    Unknown,
}
impl From<Architecture> for SerializableArchitecture {
    fn from(value: Architecture) -> Self {
        match value {
            Architecture::Arm64 => SerializableArchitecture::Arm64,
            Architecture::X8664 => SerializableArchitecture::X8664,
            _ => SerializableArchitecture::Unknown,
        }
    }
}
impl From<SerializableArchitecture> for Architecture {
    fn from(value: SerializableArchitecture) -> Self {
        match value {
            SerializableArchitecture::Arm64 => Architecture::Arm64,
            SerializableArchitecture::X8664 => Architecture::X8664,
            _ => unimplemented!(),
        }
    }
}

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct SerializableImageConfig {
    pub entry_point: Option<Vec<String>>,
    pub command: Option<Vec<String>>,
    pub working_directory: Option<String>,
}
impl From<SerializableImageConfig> for ImageConfig {
    fn from(value: SerializableImageConfig) -> Self {
        ImageConfigBuilder::default()
            .set_entry_point(value.entry_point)
            .set_command(value.command)
            .set_working_directory(value.working_directory)
            .build()
    }
}

impl From<ImageConfig> for SerializableImageConfig {
    fn from(value: ImageConfig) -> Self {
        SerializableImageConfig {
            entry_point: value.entry_point,
            command: value.command,
            working_directory: value.working_directory,
        }
    }
}

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct SerializableFileSystemConfig {
    pub arn: String,
    pub local_mount_path: String,
}
impl TryFrom<SerializableFileSystemConfig> for FileSystemConfig {
    type Error = anyhow::Error;

    fn try_from(value: SerializableFileSystemConfig) -> Result<Self, Self::Error> {
        FileSystemConfigBuilder::default()
            .set_arn(Some(value.arn))
            .set_local_mount_path(Some(value.local_mount_path))
            .build()
            .context("FilesystemConfigBuilder error")
    }
}

impl From<FileSystemConfig> for SerializableFileSystemConfig {
    fn from(value: FileSystemConfig) -> Self {
        SerializableFileSystemConfig {
            arn: value.arn,
            local_mount_path: value.local_mount_path,
        }
    }
}

#[derive(Default, Serialize, Deserialize, Debug, Clone)]

pub struct SerializableTracingConfig {
    pub mode: Option<SerializableTracingMode>,
}
impl From<SerializableTracingConfig> for TracingConfig {
    fn from(value: SerializableTracingConfig) -> Self {
        TracingConfigBuilder::default()
            .set_mode(value.mode.map(SerializableTracingMode::into))
            .build()
    }
}

impl From<TracingConfig> for SerializableTracingConfig {
    fn from(value: TracingConfig) -> Self {
        SerializableTracingConfig {
            mode: value.mode.map(SerializableTracingMode::from),
        }
    }
}
impl From<TracingConfigResponse> for SerializableTracingConfig {
    fn from(value: TracingConfigResponse) -> Self {
        SerializableTracingConfig {
            mode: value.mode.map(SerializableTracingMode::from),
        }
    }
}

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub enum SerializableTracingMode {
    #[default]
    Active,
    PassThrough,
    Unknown,
}
impl From<SerializableTracingMode> for TracingMode {
    fn from(value: SerializableTracingMode) -> Self {
        match value {
            SerializableTracingMode::Active => TracingMode::Active,
            SerializableTracingMode::PassThrough => TracingMode::PassThrough,
            _ => unimplemented!(),
        }
    }
}
impl From<TracingMode> for SerializableTracingMode {
    fn from(value: TracingMode) -> Self {
        match value {
            TracingMode::Active => SerializableTracingMode::Active,
            TracingMode::PassThrough => SerializableTracingMode::PassThrough,
            _ => SerializableTracingMode::Unknown,
        }
    }
}

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct SerializableEnvironment {
    pub variables: Option<HashMap<String, String>>,
}
impl From<SerializableEnvironment> for Environment {
    fn from(value: SerializableEnvironment) -> Self {
        EnvironmentBuilder::default()
            .set_variables(value.variables)
            .build()
    }
}

impl From<Environment> for SerializableEnvironment {
    fn from(value: Environment) -> Self {
        SerializableEnvironment {
            variables: value.variables,
        }
    }
}
impl From<EnvironmentResponse> for SerializableEnvironment {
    fn from(value: EnvironmentResponse) -> Self {
        SerializableEnvironment {
            variables: value.variables,
        }
    }
}

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct SerializableDeadLetterConfig {
    pub target_arn: Option<String>,
}
impl From<SerializableDeadLetterConfig> for DeadLetterConfig {
    fn from(value: SerializableDeadLetterConfig) -> Self {
        DeadLetterConfigBuilder::default()
            .set_target_arn(value.target_arn)
            .build()
    }
}

impl From<DeadLetterConfig> for SerializableDeadLetterConfig {
    fn from(value: DeadLetterConfig) -> Self {
        SerializableDeadLetterConfig {
            target_arn: value.target_arn,
        }
    }
}

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub enum SerializablePackageType {
    Image,
    #[default]
    Zip,
    Unknown,
}
impl From<SerializablePackageType> for PackageType {
    fn from(value: SerializablePackageType) -> Self {
        match value {
            SerializablePackageType::Image => PackageType::Image,
            SerializablePackageType::Zip => PackageType::Zip,
            _ => unimplemented!(),
        }
    }
}
impl From<PackageType> for SerializablePackageType {
    fn from(value: PackageType) -> Self {
        match value {
            PackageType::Image => SerializablePackageType::Image,
            PackageType::Zip => SerializablePackageType::Zip,
            _ => SerializablePackageType::Unknown,
        }
    }
}
#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct SerializableVpcConfig {
    pub subnet_ids: Option<Vec<String>>,
    pub security_group_ids: Option<Vec<String>>,
    pub ipv6_allowed_for_dual_stack: Option<bool>,
}

impl From<SerializableVpcConfig> for VpcConfig {
    fn from(value: SerializableVpcConfig) -> Self {
        VpcConfigBuilder::default()
            .set_subnet_ids(value.subnet_ids)
            .set_ipv6_allowed_for_dual_stack(value.ipv6_allowed_for_dual_stack)
            .set_security_group_ids(value.security_group_ids)
            .build()
    }
}

impl From<VpcConfig> for SerializableVpcConfig {
    fn from(value: VpcConfig) -> Self {
        SerializableVpcConfig {
            subnet_ids: value.subnet_ids,
            security_group_ids: value.security_group_ids,
            ipv6_allowed_for_dual_stack: value.ipv6_allowed_for_dual_stack,
        }
    }
}

impl From<VpcConfigResponse> for SerializableVpcConfig {
    fn from(value: VpcConfigResponse) -> Self {
        SerializableVpcConfig {
            subnet_ids: value.subnet_ids,
            security_group_ids: value.security_group_ids,
            ipv6_allowed_for_dual_stack: value.ipv6_allowed_for_dual_stack,
        }
    }
}

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct SerializableFunctionCode {
    pub zip_file: Option<Vec<u8>>,
    pub s3_bucket: Option<String>,
    pub s3_key: Option<String>,
    pub s3_object_version: Option<String>,
    pub image_uri: Option<String>,
}
impl From<FunctionCode> for SerializableFunctionCode {
    fn from(value: FunctionCode) -> Self {
        SerializableFunctionCode {
            zip_file: value.zip_file.map(|v| v.into_inner()),
            s3_bucket: value.s3_bucket,
            s3_key: value.s3_key,
            s3_object_version: value.s3_object_version,
            image_uri: value.image_uri,
        }
    }
}
impl From<SerializableFunctionCode> for FunctionCode {
    fn from(value: SerializableFunctionCode) -> Self {
        FunctionCodeBuilder::default()
            .set_zip_file(value.zip_file.map(Blob::new))
            .set_s3_bucket(value.s3_bucket)
            .set_s3_key(value.s3_key)
            .set_s3_object_version(value.s3_object_version)
            .set_image_uri(value.image_uri)
            .build()
    }
}

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub enum SerializableRuntime {
    Dotnet6,
    Dotnet8,
    Dotnetcore10,
    Dotnetcore20,
    Dotnetcore21,
    Dotnetcore31,
    Go1x,
    Java11,
    Java17,
    Java21,
    Java8,
    Java8al2,
    Nodejs,
    Nodejs10x,
    Nodejs12x,
    Nodejs14x,
    Nodejs16x,
    Nodejs18x,
    Nodejs20x,
    Nodejs43,
    Nodejs43edge,
    Nodejs610,
    Nodejs810,
    #[default]
    Provided,
    Providedal2,
    Providedal2023,
    Python27,
    Python310,
    Python311,
    Python312,
    Python36,
    Python37,
    Python38,
    Python39,
    Ruby25,
    Ruby27,
    Ruby32,
    Ruby33,
    Unknown,
}
impl From<SerializableRuntime> for Runtime {
    fn from(value: SerializableRuntime) -> Self {
        match value {
            SerializableRuntime::Dotnet6 => Runtime::Dotnet6,
            SerializableRuntime::Dotnet8 => Runtime::Dotnet8,
            SerializableRuntime::Dotnetcore10 => Runtime::Dotnetcore10,
            SerializableRuntime::Dotnetcore20 => Runtime::Dotnetcore20,
            SerializableRuntime::Dotnetcore21 => Runtime::Dotnetcore21,
            SerializableRuntime::Dotnetcore31 => Runtime::Dotnetcore31,
            SerializableRuntime::Go1x => Runtime::Go1x,
            SerializableRuntime::Java11 => Runtime::Java11,
            SerializableRuntime::Java17 => Runtime::Java17,
            SerializableRuntime::Java21 => Runtime::Java21,
            SerializableRuntime::Java8 => Runtime::Java8,
            SerializableRuntime::Java8al2 => Runtime::Java8al2,
            SerializableRuntime::Nodejs => Runtime::Nodejs,
            SerializableRuntime::Nodejs10x => Runtime::Nodejs10x,
            SerializableRuntime::Nodejs12x => Runtime::Nodejs12x,
            SerializableRuntime::Nodejs14x => Runtime::Nodejs14x,
            SerializableRuntime::Nodejs16x => Runtime::Nodejs16x,
            SerializableRuntime::Nodejs18x => Runtime::Nodejs18x,
            SerializableRuntime::Nodejs20x => Runtime::Nodejs20x,
            SerializableRuntime::Nodejs43 => Runtime::Nodejs43,
            SerializableRuntime::Nodejs43edge => Runtime::Nodejs43edge,
            SerializableRuntime::Nodejs610 => Runtime::Nodejs610,
            SerializableRuntime::Nodejs810 => Runtime::Nodejs810,
            SerializableRuntime::Provided => Runtime::Provided,
            SerializableRuntime::Providedal2 => Runtime::Providedal2,
            SerializableRuntime::Providedal2023 => Runtime::Providedal2023,
            SerializableRuntime::Python27 => Runtime::Python27,
            SerializableRuntime::Python310 => Runtime::Python310,
            SerializableRuntime::Python311 => Runtime::Python311,
            SerializableRuntime::Python312 => Runtime::Python312,
            SerializableRuntime::Python36 => Runtime::Python36,
            SerializableRuntime::Python37 => Runtime::Python37,
            SerializableRuntime::Python38 => Runtime::Python38,
            SerializableRuntime::Python39 => Runtime::Python39,
            SerializableRuntime::Ruby25 => Runtime::Ruby25,
            SerializableRuntime::Ruby27 => Runtime::Ruby27,
            SerializableRuntime::Ruby32 => Runtime::Ruby32,
            SerializableRuntime::Ruby33 => Runtime::Ruby33,
            _ => unimplemented!(),
        }
    }
}
impl From<Runtime> for SerializableRuntime {
    fn from(value: Runtime) -> Self {
        match value {
            Runtime::Dotnet6 => SerializableRuntime::Dotnet6,
            Runtime::Dotnet8 => SerializableRuntime::Dotnet8,
            Runtime::Dotnetcore10 => SerializableRuntime::Dotnetcore10,
            Runtime::Dotnetcore20 => SerializableRuntime::Dotnetcore20,
            Runtime::Dotnetcore21 => SerializableRuntime::Dotnetcore21,
            Runtime::Dotnetcore31 => SerializableRuntime::Dotnetcore31,
            Runtime::Go1x => SerializableRuntime::Go1x,
            Runtime::Java11 => SerializableRuntime::Java11,
            Runtime::Java17 => SerializableRuntime::Java17,
            Runtime::Java21 => SerializableRuntime::Java21,
            Runtime::Java8 => SerializableRuntime::Java8,
            Runtime::Java8al2 => SerializableRuntime::Java8al2,
            Runtime::Nodejs => SerializableRuntime::Nodejs,
            Runtime::Nodejs10x => SerializableRuntime::Nodejs10x,
            Runtime::Nodejs12x => SerializableRuntime::Nodejs12x,
            Runtime::Nodejs14x => SerializableRuntime::Nodejs14x,
            Runtime::Nodejs16x => SerializableRuntime::Nodejs16x,
            Runtime::Nodejs18x => SerializableRuntime::Nodejs18x,
            Runtime::Nodejs20x => SerializableRuntime::Nodejs20x,
            Runtime::Nodejs43 => SerializableRuntime::Nodejs43,
            Runtime::Nodejs43edge => SerializableRuntime::Nodejs43edge,
            Runtime::Nodejs610 => SerializableRuntime::Nodejs610,
            Runtime::Nodejs810 => SerializableRuntime::Nodejs810,
            Runtime::Provided => SerializableRuntime::Provided,
            Runtime::Providedal2 => SerializableRuntime::Providedal2,
            Runtime::Providedal2023 => SerializableRuntime::Providedal2023,
            Runtime::Python27 => SerializableRuntime::Python27,
            Runtime::Python310 => SerializableRuntime::Python310,
            Runtime::Python311 => SerializableRuntime::Python311,
            Runtime::Python312 => SerializableRuntime::Python312,
            Runtime::Python36 => SerializableRuntime::Python36,
            Runtime::Python37 => SerializableRuntime::Python37,
            Runtime::Python38 => SerializableRuntime::Python38,
            Runtime::Python39 => SerializableRuntime::Python39,
            Runtime::Ruby25 => SerializableRuntime::Ruby25,
            Runtime::Ruby27 => SerializableRuntime::Ruby27,
            Runtime::Ruby32 => SerializableRuntime::Ruby32,
            Runtime::Ruby33 => SerializableRuntime::Ruby33,
            _ => SerializableRuntime::Unknown,
        }
    }
}
