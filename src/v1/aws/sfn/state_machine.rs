use crate::{
    prelude::{ArnProvider, AwsManager, AwsResource, AwsResourceCreator, Role, SerializableTag},
    v1::manager::{ManagerError, ResourceManager},
};

use super::schema::{State, StateMachineDocument};
use aws_sdk_sfn::{
    operation::{
        create_state_machine::builders::CreateStateMachineFluentBuilder,
        describe_state_machine::DescribeStateMachineOutput,
        update_state_machine::builders::UpdateStateMachineFluentBuilder,
    },
    types::{
        CloudWatchLogsLogGroup, LogDestination, LogLevel, LoggingConfiguration, StateMachineStatus,
        StateMachineType, TracingConfiguration,
    },
    Client,
};
use serde::{Deserialize, Serialize};
use serde_json::Error;

pub type StateMachineOutput = SerializableStateMachine;
pub type StateMachineInput = SerializableCreateStateMachineInput;
pub type StateMachineManager = AwsManager<StateMachineInput, StateMachineOutput, Client>;
pub type StateMachine<'a> = AwsResource<'a, StateMachineInput, StateMachineOutput>;

impl AwsResourceCreator for StateMachine<'_> {
    type Input = StateMachineInput;
    type Output = StateMachineOutput;
    fn r#type() -> crate::prelude::AwsType {
        crate::prelude::AwsType::StateMachine
    }
    fn manager(
        handle: &tokio::runtime::Handle,
        config: &aws_config::SdkConfig,
    ) -> std::sync::Arc<dyn ResourceManager<Self::Input, Self::Output>> {
        StateMachineManager::new(handle, Client::new(config), config).arc()
    }
    fn input_hook(id: &str, input: &mut Self::Input) {
        if input.name.is_none() {
            input.name = Some(id.to_string())
        }
    }
}

impl ArnProvider for StateMachineOutput {
    fn arn(&self) -> Option<Vec<String>> {
        Some(vec![self.state_machine_arn.clone()])
    }
}

impl<'a> StateMachine<'a> {
    pub fn bind_role(self, role: &Role) -> anyhow::Result<StateMachine<'a>> {
        self.bind(role, |this, other| this.role_arn = Some(other.arn.clone()))?;
        Ok(self)
    }

    pub fn bind_function<Input: Clone + 'static, Output: Clone + ArnProvider + 'static>(
        self,
        state_name: impl Into<String>,
        resource: &AwsResource<Input, Output>,
    ) -> anyhow::Result<StateMachine<'a>> {
        let state_name = state_name.into();
        match self.inner.resource.lock() {
            Ok(input) => match input.input.definition.as_ref() {
                Some(definition) => {
                    if !definition.states.contains_key(&state_name) {
                        return Err(anyhow::anyhow!(format!(
                            "State {} was not found in the state machine definition",
                            state_name
                        )));
                    }
                }
                None => return Err(anyhow::anyhow!("State machine has no definition")),
            },
            Err(_) => return Err(anyhow::anyhow!("Could not lock input")),
        }
        self.bind(resource, move |this, other| {
            if let Some(arn) = other.arn() {
                if let Some(definition) = this.definition.as_mut() {
                    if let Some(State::Task(task)) = definition.states.get_mut(&state_name) {
                        task.resource.clone_from(&arn[0]);
                    }
                }
            }
        })?;
        Ok(self)
    }

    pub fn get_arn(&self) -> Option<String> {
        self.inner.resource.lock().ok().and_then(|input| {
            input
                .output
                .as_ref()
                .map(|output| output.state_machine_arn.clone())
        })
    }
}

impl StateMachineManager {
    fn get_arn(&self, input: &StateMachineInput) -> Result<String, ManagerError> {
        let account_id = self.get_identity()?.account.unwrap_or("".to_string());
        let region = self.get_region()?;
        Ok(format!(
            "arn:aws:states:{}:{}:stateMachine:{}",
            region,
            account_id,
            input.name.as_ref().unwrap_or(&"".to_string())
        ))
    }
    fn lookup(&self, arn: &str) -> Result<Option<StateMachineOutput>, ManagerError> {
        self.handle.block_on(async {
            self.client
                .describe_state_machine()
                .set_state_machine_arn(Some(arn.to_string()))
                .send()
                .await
                .map_err(|e| ManagerError::LookupFail(format!("{:?}", e.into_source())))
                .and_then(|output| {
                    StateMachineOutput::try_from(output)
                        .map_err(|e| ManagerError::LookupFail(format!("{:?}", e)))
                })
                .map(Some)
                .or_else(|e| match e {
                    ManagerError::LookupFail(ref msg) if msg.contains("Does Not Exist") => Ok(None),
                    _ => Err(e),
                })
        })
    }
    fn create(&self, input: &StateMachineInput) -> Result<StateMachineOutput, ManagerError> {
        let output = self.handle.block_on(async {
            input
                .clone()
                .to_aws_input(&self.client)
                .map_err(|e| ManagerError::CreateFail(format!("{:?}", e)))?
                .send()
                .await
                .map_err(|e| ManagerError::CreateFail(format!("{:?}", e.into_source())))
        })?;
        if let Some(output) = self.lookup(&output.state_machine_arn)? {
            Ok(output)
        } else {
            Err(ManagerError::CreateFail(
                "State machine has been created but could not be retrieved".to_string(),
            ))
        }
    }
    fn update(
        &self,
        input: &StateMachineInput,
        latest: &StateMachineOutput,
    ) -> Result<StateMachineOutput, ManagerError> {
        self.handle.block_on(async {
            input
                .clone()
                .to_aws_update(Some(latest.state_machine_arn.clone()), &self.client)
                .map_err(|e| ManagerError::UpdateFail(format!("{:?}", e)))?
                .send()
                .await
                .map_err(|e| ManagerError::CreateFail(format!("{:?}", e.into_source())))
        })?;
        if let Some(output) = self.lookup(&latest.state_machine_arn)? {
            Ok(output)
        } else {
            Err(ManagerError::CreateFail(
                "State machine has been created but could not be retrieved".to_string(),
            ))
        }
    }

    fn delete(&self, arn: &str) -> Result<bool, ManagerError> {
        self.handle.block_on(async {
            self.client
                .delete_state_machine()
                .set_state_machine_arn(Some(arn.to_string()))
                .send()
                .await
                .map_err(|e| ManagerError::LookupFail(format!("{:?}", e.into_source())))
                .map(|_| true)
        })
    }
}

impl ResourceManager<StateMachineInput, StateMachineOutput> for StateMachineManager {
    fn lookup(
        &self,
        latest: &StateMachineOutput,
    ) -> Result<Option<StateMachineOutput>, crate::v1::manager::ManagerError> {
        self.lookup(&latest.state_machine_arn)
    }

    fn lookup_by_input(
        &self,
        input: &StateMachineInput,
    ) -> Result<Option<StateMachineOutput>, crate::v1::manager::ManagerError> {
        self.lookup(&self.get_arn(input)?)
    }

    fn create(
        &self,
        input: &mut StateMachineInput,
    ) -> Result<StateMachineOutput, crate::v1::manager::ManagerError> {
        self.create(input)
    }

    fn delete(
        &self,
        latest: &StateMachineOutput,
    ) -> Result<bool, crate::v1::manager::ManagerError> {
        self.delete(&latest.state_machine_arn)
    }

    fn syncup(
        &self,
        latest: &StateMachineOutput,
        input: &mut StateMachineInput,
    ) -> Result<Option<StateMachineOutput>, crate::v1::manager::ManagerError> {
        self.update(input, latest).map(Some)
    }
}
#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct SerializableCreateStateMachineInput {
    pub name: Option<String>,
    pub definition: Option<StateMachineDocument>,
    pub role_arn: Option<String>,
    pub r#type: Option<SerializableStateMachineType>,
    pub logging_configuration: Option<SerializableLoggingConfigurationForSM>,
    pub tags: Option<Vec<SerializableTag>>,
    pub tracing_configuration: Option<SerializableTracingConfiguration>,
    pub publish: Option<bool>,
    pub version_description: Option<String>,
}

impl SerializableCreateStateMachineInput {
    pub fn definition(&self) -> anyhow::Result<Option<String>> {
        if let Some(def) = self.definition.as_ref() {
            Ok(Some(serde_json::to_string(&def)?).inspect(|j| println!("{}", j)))
        } else {
            Ok(None)
        }
    }
    pub fn to_aws_input(
        self,
        client: &aws_sdk_sfn::Client,
    ) -> anyhow::Result<CreateStateMachineFluentBuilder> {
        Ok(client
            .create_state_machine()
            .set_definition(self.definition()?)
            .set_name(self.name)
            .set_role_arn(self.role_arn)
            .set_type(self.r#type.map(StateMachineType::from)))
    }
    pub fn to_aws_update(
        self,
        arn: Option<String>,
        client: &aws_sdk_sfn::Client,
    ) -> anyhow::Result<UpdateStateMachineFluentBuilder> {
        Ok(client
            .update_state_machine()
            .set_definition(self.definition()?)
            .set_role_arn(self.role_arn)
            .set_state_machine_arn(arn))
    }
}

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct SerializableStateMachine {
    pub state_machine_arn: String,
    pub name: String,
    pub status: Option<SerializableStateMachineStatus>,
    pub definition: StateMachineDocument,
    pub role_arn: String,
    pub r#type: SerializableStateMachineType,
    pub logging_configuration: Option<SerializableLoggingConfigurationForSM>,
    pub tracing_configuration: Option<SerializableTracingConfiguration>,
    pub label: Option<String>,
    pub revision_id: Option<String>,
    pub description: Option<String>,
}
impl TryFrom<DescribeStateMachineOutput> for SerializableStateMachine {
    type Error = Error;
    fn try_from(value: DescribeStateMachineOutput) -> Result<Self, Self::Error> {
        Ok(Self {
            state_machine_arn: value.state_machine_arn,
            name: value.name,
            status: value.status.map(SerializableStateMachineStatus::from),
            definition: serde_json::from_str(&value.definition)?,
            role_arn: value.role_arn,
            r#type: value.r#type.into(),
            logging_configuration: value
                .logging_configuration
                .map(SerializableLoggingConfigurationForSM::from),
            tracing_configuration: value
                .tracing_configuration
                .map(SerializableTracingConfiguration::from),
            label: value.label,
            revision_id: value.revision_id,
            description: value.description,
        })
    }
}

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct SerializableTracingConfiguration {
    pub enabled: bool,
}
impl From<TracingConfiguration> for SerializableTracingConfiguration {
    fn from(value: TracingConfiguration) -> Self {
        Self {
            enabled: value.enabled,
        }
    }
}

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct SerializableLoggingConfigurationForSM {
    pub level: Option<SerializableLogLevelForSM>,
    pub include_execution_data: bool,
    pub destinations: Option<Vec<SerializableLogDestinationForSM>>,
}

impl From<LoggingConfiguration> for SerializableLoggingConfigurationForSM {
    fn from(value: LoggingConfiguration) -> Self {
        Self {
            level: value.level.map(SerializableLogLevelForSM::from),
            include_execution_data: value.include_execution_data,
            destinations: value.destinations.map(|destinations| {
                destinations
                    .into_iter()
                    .map(SerializableLogDestinationForSM::from)
                    .collect()
            }),
        }
    }
}
#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct SerializableLogDestinationForSM {
    pub cloud_watch_logs_log_group: Option<SerializableCloudWatchLogsLogGroupForSM>,
}
impl From<LogDestination> for SerializableLogDestinationForSM {
    fn from(value: LogDestination) -> Self {
        Self {
            cloud_watch_logs_log_group: value
                .cloud_watch_logs_log_group
                .map(SerializableCloudWatchLogsLogGroupForSM::from),
        }
    }
}

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct SerializableCloudWatchLogsLogGroupForSM {
    pub log_group_arn: Option<String>,
}

impl From<CloudWatchLogsLogGroup> for SerializableCloudWatchLogsLogGroupForSM {
    fn from(value: CloudWatchLogsLogGroup) -> Self {
        Self {
            log_group_arn: value.log_group_arn,
        }
    }
}
#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub enum SerializableLogLevelForSM {
    All,
    #[default]
    Error,
    Fatal,
    Off,
    Unknown,
}

impl From<LogLevel> for SerializableLogLevelForSM {
    fn from(value: LogLevel) -> Self {
        match value {
            LogLevel::All => Self::All,
            LogLevel::Error => Self::Error,
            LogLevel::Fatal => Self::Fatal,
            LogLevel::Off => Self::Off,
            _ => Self::Unknown,
        }
    }
}

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub enum SerializableStateMachineStatus {
    #[default]
    Active,
    Deleting,
    Unknown,
}
impl From<StateMachineStatus> for SerializableStateMachineStatus {
    fn from(value: StateMachineStatus) -> Self {
        match value {
            StateMachineStatus::Active => Self::Active,
            StateMachineStatus::Deleting => Self::Deleting,
            _ => Self::Unknown,
        }
    }
}
impl From<SerializableStateMachineStatus> for StateMachineStatus {
    fn from(value: SerializableStateMachineStatus) -> Self {
        match value {
            SerializableStateMachineStatus::Active => Self::Active,
            SerializableStateMachineStatus::Deleting => Self::Deleting,
            SerializableStateMachineStatus::Unknown => Self::Active,
        }
    }
}

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub enum SerializableStateMachineType {
    #[default]
    Express,
    Standard,
    Unknown,
}
impl From<SerializableStateMachineType> for StateMachineType {
    fn from(value: SerializableStateMachineType) -> Self {
        match value {
            SerializableStateMachineType::Express => StateMachineType::Express,
            SerializableStateMachineType::Standard => StateMachineType::Standard,
            SerializableStateMachineType::Unknown => StateMachineType::Express,
        }
    }
}

impl From<StateMachineType> for SerializableStateMachineType {
    fn from(value: StateMachineType) -> Self {
        match value {
            StateMachineType::Standard => Self::Standard,
            StateMachineType::Express => Self::Express,
            _ => Self::Unknown,
        }
    }
}
