use std::{
    collections::HashMap,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::Context;
use aws_sdk_s3::primitives::DateTime;
use aws_sdk_scheduler::{
    operation::{
        create_schedule::builders::CreateScheduleFluentBuilder, get_schedule::GetScheduleOutput,
        update_schedule::builders::UpdateScheduleFluentBuilder,
    },
    types::{
        builders::{
            AwsVpcConfigurationBuilder, CapacityProviderStrategyItemBuilder,
            DeadLetterConfigBuilder, EcsParametersBuilder, EventBridgeParametersBuilder,
            FlexibleTimeWindowBuilder, KinesisParametersBuilder, NetworkConfigurationBuilder,
            PlacementConstraintBuilder, PlacementStrategyBuilder, RetryPolicyBuilder,
            SageMakerPipelineParameterBuilder, SageMakerPipelineParametersBuilder,
            SqsParametersBuilder, TargetBuilder,
        },
        ActionAfterCompletion, AssignPublicIp, AwsVpcConfiguration, CapacityProviderStrategyItem,
        DeadLetterConfig, EcsParameters, EventBridgeParameters, FlexibleTimeWindow,
        FlexibleTimeWindowMode, KinesisParameters, LaunchType, NetworkConfiguration,
        PlacementConstraint, PlacementConstraintType, PlacementStrategy, PlacementStrategyType,
        PropagateTags, RetryPolicy, SageMakerPipelineParameter, SageMakerPipelineParameters,
        ScheduleState, SqsParameters, Target,
    },
    Client,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::prelude::{ArnProvider, AwsManager, ResourceError, Role, StateMachine};
use crate::{
    prelude::{AwsResource, AwsResourceCreator},
    v1::manager::{ManagerError, ResourceManager},
};

pub type ScheduleInput = SerializableCreateScheduleInput;
pub type ScheduleOutput = SerializableGetScheduleOutput;
pub type ScheduleManager = AwsManager<ScheduleInput, ScheduleOutput, Client>;
pub type Schedule<'a> = AwsResource<'a, ScheduleInput, ScheduleOutput>;

#[derive(Serialize, Deserialize, Debug, Clone)]
struct StateMachineTargetInput {
    #[serde(rename = "Input")]
    input: String,
    #[serde(rename = "StateMachineArn")]
    state_machine_arn: String,
}
impl Schedule<'_> {
    pub fn set_state_machine_target(
        &self,
        sm: &StateMachine,
        input: Value,
    ) -> Result<(), ResourceError> {
        let input = serde_json::to_string(&input)
            .map_err(|e| ResourceError::BindingFail(format!("{:?}", e)))?;
        self.bind(sm, move |this, other| {
            let input = StateMachineTargetInput {
                input: input.clone(),
                state_machine_arn: other.state_machine_arn.clone(),
            };
            let sm_input = serde_json::to_string(&input).unwrap();
            let target_arn = "arn:aws:scheduler:::aws-sdk:sfn:startExecution".to_string();
            if let Some(target) = this.target.as_mut() {
                target.arn = target_arn;
                target.input = Some(sm_input);
            } else {
                this.target = Some(SerializableTarget {
                    arn: target_arn,
                    input: Some(sm_input),
                    ..Default::default()
                });
            }
        })
    }
    pub fn set_target_role(&self, role: &Role) -> Result<(), ResourceError> {
        self.bind(role, |this, other| {
            if let Some(target) = this.target.as_mut() {
                target.role_arn.clone_from(&other.arn);
            } else {
                let target = SerializableTarget {
                    role_arn: other.arn.clone(),
                    ..Default::default()
                };
                this.target = Some(target);
            }
        })
    }
}

impl AwsResourceCreator for Schedule<'_> {
    type Input = ScheduleInput;
    type Output = ScheduleOutput;
    fn r#type() -> crate::prelude::AwsType {
        crate::prelude::AwsType::Schedule
    }
    fn manager(
        handle: &tokio::runtime::Handle,
        config: &aws_config::SdkConfig,
    ) -> std::sync::Arc<dyn ResourceManager<Self::Input, Self::Output>> {
        ScheduleManager::new(handle, Client::new(config), config).arc()
    }

    fn input_hook(_id: &str, _input: &mut Self::Input) {}
}
impl ArnProvider for ScheduleOutput {
    fn arn(&self) -> Option<Vec<String>> {
        Some(vec![self.arn.clone().unwrap_or_default()])
    }
}
impl ScheduleManager {
    fn lookup(
        &self,
        name: Option<String>,
        group_name: Option<String>,
    ) -> Result<Option<ScheduleOutput>, crate::v1::manager::ManagerError> {
        self.handle.block_on(async {
            self.client
                .get_schedule()
                .set_name(name)
                .set_group_name(group_name)
                .send()
                .await
                .map_err(|e| ManagerError::LookupFail(format!("{:?}", e.into_source())))
                .map(ScheduleOutput::from)
                .map(Some)
                .or_else(|e| match e {
                    ManagerError::LookupFail(ref msg) if msg.contains("NotFound") => Ok(None),
                    _ => Err(e),
                })
        })
    }
    fn create(
        &self,
        input: &mut ScheduleInput,
    ) -> Result<ScheduleOutput, crate::v1::manager::ManagerError> {
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
            Ok(Some(policy)) => Ok(policy),
            Ok(None) => Err(ManagerError::CreateFail(
                "Policy create but not found".to_string(),
            )),
            Err(e) => Err(e),
        }
    }

    fn delete(&self, latest: &ScheduleOutput) -> Result<bool, crate::v1::manager::ManagerError> {
        self.handle.block_on(async {
            self.client
                .delete_schedule()
                .set_name(latest.name.clone())
                .set_group_name(latest.group_name.clone())
                .send()
                .await
                .map_err(|e| ManagerError::DeleteFail(format!("{:?}", e.into_source())))
                .map(|_| true)
        })
    }

    fn syncup(
        &self,
        _latest: &ScheduleOutput,
        input: &mut ScheduleInput,
    ) -> Result<Option<ScheduleOutput>, crate::v1::manager::ManagerError> {
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
                "Policy create but not found".to_string(),
            )),
            Err(e) => Err(e),
        }
    }
}

impl ResourceManager<ScheduleInput, ScheduleOutput> for ScheduleManager {
    fn lookup(&self, latest: &ScheduleOutput) -> Result<Option<ScheduleOutput>, ManagerError> {
        let name = latest.name.clone();
        let group_name = latest.group_name.clone();
        self.lookup(name, group_name)
    }

    fn lookup_by_input(
        &self,
        input: &ScheduleInput,
    ) -> Result<Option<ScheduleOutput>, ManagerError> {
        let name = input.name.clone();
        let group_name = input.group_name.clone();
        self.lookup(name, group_name)
    }

    fn create(&self, input: &mut ScheduleInput) -> Result<ScheduleOutput, ManagerError> {
        self.create(input)
    }

    fn delete(&self, latest: &ScheduleOutput) -> Result<bool, ManagerError> {
        self.delete(latest)
    }

    fn syncup(
        &self,
        latest: &ScheduleOutput,
        input: &mut ScheduleInput,
    ) -> Result<Option<ScheduleOutput>, ManagerError> {
        self.syncup(latest, input)
    }
}

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct SerializableCreateScheduleInput {
    pub name: Option<String>,
    pub group_name: Option<String>,
    pub schedule_expression: Option<String>,
    pub start_date: Option<SystemTime>,
    pub end_date: Option<SystemTime>,
    pub description: Option<String>,
    pub schedule_expression_timezone: Option<String>,
    pub state: Option<SerializableScheduleState>,
    pub kms_key_arn: Option<String>,
    pub target: Option<SerializableTarget>,
    pub flexible_time_window: SerializableFlexibleTimeWindow,
    pub client_token: Option<String>,
    pub action_after_completion: Option<SerializableActionAfterCompletion>,
}
impl SerializableCreateScheduleInput {
    pub fn to_aws_input(self, client: &Client) -> anyhow::Result<CreateScheduleFluentBuilder> {
        Ok(client
            .create_schedule()
            .set_action_after_completion(
                self.action_after_completion
                    .map(ActionAfterCompletion::from),
            )
            .set_client_token(self.client_token)
            .set_description(self.description)
            .set_end_date(self.end_date.map(DateTime::from))
            .set_flexible_time_window(Some(FlexibleTimeWindow::try_from(
                self.flexible_time_window,
            )?))
            .set_group_name(self.group_name)
            .set_kms_key_arn(self.kms_key_arn)
            .set_name(self.name)
            .set_schedule_expression(self.schedule_expression)
            .set_schedule_expression_timezone(self.schedule_expression_timezone)
            .set_start_date(self.start_date.map(DateTime::from))
            .set_state(self.state.map(ScheduleState::from))
            .set_target(if let Some(target) = self.target.map(Target::try_from) {
                Some(target?)
            } else {
                None
            }))
    }
    pub fn to_aws_update(self, client: &Client) -> anyhow::Result<UpdateScheduleFluentBuilder> {
        Ok(client
            .update_schedule()
            .set_action_after_completion(
                self.action_after_completion
                    .map(ActionAfterCompletion::from),
            )
            .set_client_token(self.client_token)
            .set_description(self.description)
            .set_end_date(self.end_date.map(DateTime::from))
            .set_flexible_time_window(Some(FlexibleTimeWindow::try_from(
                self.flexible_time_window,
            )?))
            .set_group_name(self.group_name)
            .set_kms_key_arn(self.kms_key_arn)
            .set_name(self.name)
            .set_schedule_expression(self.schedule_expression)
            .set_schedule_expression_timezone(self.schedule_expression_timezone)
            .set_start_date(self.start_date.map(DateTime::from))
            .set_state(self.state.map(ScheduleState::from))
            .set_target(if let Some(target) = self.target.map(Target::try_from) {
                Some(target?)
            } else {
                None
            }))
    }
}

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct SerializableGetScheduleOutput {
    pub arn: Option<String>,
    pub group_name: Option<String>,
    pub name: Option<String>,
    pub schedule_expression: Option<String>,
    pub start_date: Option<SystemTime>,
    pub end_date: Option<SystemTime>,
    pub description: Option<String>,
    pub schedule_expression_timezone: Option<String>,
    pub state: Option<SerializableScheduleState>,
    pub creation_date: Option<SystemTime>,
    pub last_modification_date: Option<SystemTime>,
    pub kms_key_arn: Option<String>,
    pub target: Option<SerializableTarget>,
    pub flexible_time_window: Option<SerializableFlexibleTimeWindow>,
    pub action_after_completion: Option<SerializableActionAfterCompletion>,
}

fn convert_datetime_to_system_time(datetime: DateTime) -> SystemTime {
    UNIX_EPOCH + Duration::new(datetime.secs() as u64, datetime.subsec_nanos())
}
impl From<GetScheduleOutput> for SerializableGetScheduleOutput {
    fn from(output: GetScheduleOutput) -> Self {
        Self {
            arn: output.arn,
            group_name: output.group_name,
            name: output.name,
            schedule_expression: output.schedule_expression,
            start_date: output.start_date.map(convert_datetime_to_system_time),
            end_date: output.end_date.map(convert_datetime_to_system_time),
            description: output.description,
            schedule_expression_timezone: output.schedule_expression_timezone,
            state: output.state.map(SerializableScheduleState::from),
            creation_date: output.creation_date.map(convert_datetime_to_system_time),
            last_modification_date: output
                .last_modification_date
                .map(convert_datetime_to_system_time),
            kms_key_arn: output.kms_key_arn,
            target: output.target.map(SerializableTarget::from),
            flexible_time_window: output
                .flexible_time_window
                .map(SerializableFlexibleTimeWindow::from),
            action_after_completion: output
                .action_after_completion
                .map(SerializableActionAfterCompletion::from),
        }
    }
}

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub enum SerializableActionAfterCompletion {
    Delete,
    #[default]
    None,
    Unknown,
}
impl From<ActionAfterCompletion> for SerializableActionAfterCompletion {
    fn from(action: ActionAfterCompletion) -> Self {
        match action {
            ActionAfterCompletion::Delete => Self::Delete,
            ActionAfterCompletion::None => Self::None,
            _ => Self::Unknown,
        }
    }
}
impl From<SerializableActionAfterCompletion> for ActionAfterCompletion {
    fn from(action: SerializableActionAfterCompletion) -> Self {
        match action {
            SerializableActionAfterCompletion::Delete => ActionAfterCompletion::Delete,
            SerializableActionAfterCompletion::None => ActionAfterCompletion::None,
            _ => ActionAfterCompletion::None,
        }
    }
}
#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct SerializableFlexibleTimeWindow {
    pub mode: SerializableFlexibleTimeWindowMode,
    pub maximum_window_in_minutes: Option<i32>,
}
impl TryFrom<SerializableFlexibleTimeWindow> for FlexibleTimeWindow {
    type Error = anyhow::Error;
    fn try_from(window: SerializableFlexibleTimeWindow) -> Result<Self, Self::Error> {
        FlexibleTimeWindowBuilder::default()
            .set_mode(Some(window.mode.into()))
            .set_maximum_window_in_minutes(window.maximum_window_in_minutes)
            .build()
            .context("Failed to build FlexibleTimeWindow from SerializableFlexibleTimeWindow")
    }
}
impl From<FlexibleTimeWindow> for SerializableFlexibleTimeWindow {
    fn from(window: FlexibleTimeWindow) -> Self {
        Self {
            mode: window.mode.into(),
            maximum_window_in_minutes: window.maximum_window_in_minutes,
        }
    }
}
#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub enum SerializableFlexibleTimeWindowMode {
    Flexible,
    #[default]
    Off,
    Unknown,
}
impl From<SerializableFlexibleTimeWindowMode> for FlexibleTimeWindowMode {
    fn from(mode: SerializableFlexibleTimeWindowMode) -> Self {
        match mode {
            SerializableFlexibleTimeWindowMode::Flexible => FlexibleTimeWindowMode::Flexible,
            SerializableFlexibleTimeWindowMode::Off => FlexibleTimeWindowMode::Off,
            _ => FlexibleTimeWindowMode::Off,
        }
    }
}
impl From<FlexibleTimeWindowMode> for SerializableFlexibleTimeWindowMode {
    fn from(mode: FlexibleTimeWindowMode) -> Self {
        match mode {
            FlexibleTimeWindowMode::Flexible => Self::Flexible,
            FlexibleTimeWindowMode::Off => Self::Off,
            _ => Self::Unknown,
        }
    }
}
#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub enum SerializableScheduleState {
    #[default]
    Disabled,
    Enabled,
    Unknown,
}

impl From<SerializableScheduleState> for ScheduleState {
    fn from(state: SerializableScheduleState) -> Self {
        match state {
            SerializableScheduleState::Disabled => ScheduleState::Disabled,
            SerializableScheduleState::Enabled => ScheduleState::Enabled,
            _ => ScheduleState::Disabled,
        }
    }
}

impl From<ScheduleState> for SerializableScheduleState {
    fn from(state: ScheduleState) -> Self {
        match state {
            ScheduleState::Disabled => Self::Disabled,
            ScheduleState::Enabled => Self::Enabled,
            _ => Self::Unknown,
        }
    }
}

#[derive(Default, Serialize, Deserialize, Debug, Clone)]

pub struct SerializableTarget {
    pub arn: String,
    pub role_arn: String,
    pub dead_letter_config: Option<SerializableSDeadLetterConfig>,
    pub retry_policy: Option<SerializableRetryPolicy>,
    pub input: Option<String>,
    pub ecs_parameters: Option<SerializableEcsParameters>,
    pub event_bridge_parameters: Option<SerializableEventBridgeParameters>,
    pub kinesis_parameters: Option<SerializableKinesisParameters>,
    pub sage_maker_pipeline_parameters: Option<SerializableSageMakerPipelineParameters>,
    pub sqs_parameters: Option<SerializableSqsParameters>,
}
impl TryFrom<SerializableTarget> for Target {
    type Error = anyhow::Error;
    fn try_from(target: SerializableTarget) -> Result<Self, Self::Error> {
        TargetBuilder::default()
            .set_arn(Some(target.arn))
            .set_role_arn(Some(target.role_arn))
            .set_dead_letter_config(
                if let Some(r) = target.dead_letter_config.map(DeadLetterConfig::try_from) {
                    Some(r?)
                } else {
                    None
                },
            )
            .set_retry_policy(
                if let Some(r) = target.retry_policy.map(RetryPolicy::try_from) {
                    Some(r?)
                } else {
                    None
                },
            )
            .set_input(target.input)
            .set_ecs_parameters(
                if let Some(r) = target.ecs_parameters.map(EcsParameters::try_from) {
                    Some(r?)
                } else {
                    None
                },
            )
            .set_event_bridge_parameters(
                if let Some(r) = target
                    .event_bridge_parameters
                    .map(EventBridgeParameters::try_from)
                {
                    Some(r?)
                } else {
                    None
                },
            )
            .set_kinesis_parameters(
                if let Some(r) = target.kinesis_parameters.map(KinesisParameters::try_from) {
                    Some(r?)
                } else {
                    None
                },
            )
            .set_sage_maker_pipeline_parameters(
                if let Some(r) = target
                    .sage_maker_pipeline_parameters
                    .map(SageMakerPipelineParameters::try_from)
                {
                    Some(r?)
                } else {
                    None
                },
            )
            .set_sqs_parameters(
                if let Some(r) = target.sqs_parameters.map(SqsParameters::try_from) {
                    Some(r?)
                } else {
                    None
                },
            )
            .build()
            .context("Failed to build Target from SerializableTarget")
    }
}

impl From<Target> for SerializableTarget {
    fn from(target: Target) -> Self {
        Self {
            arn: target.arn,
            role_arn: target.role_arn,
            dead_letter_config: target
                .dead_letter_config
                .map(SerializableSDeadLetterConfig::from),
            retry_policy: target.retry_policy.map(SerializableRetryPolicy::from),
            input: target.input,
            ecs_parameters: target.ecs_parameters.map(SerializableEcsParameters::from),
            event_bridge_parameters: target
                .event_bridge_parameters
                .map(SerializableEventBridgeParameters::from),
            kinesis_parameters: target
                .kinesis_parameters
                .map(SerializableKinesisParameters::from),
            sage_maker_pipeline_parameters: target
                .sage_maker_pipeline_parameters
                .map(SerializableSageMakerPipelineParameters::from),
            sqs_parameters: target.sqs_parameters.map(SerializableSqsParameters::from),
        }
    }
}

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct SerializableSqsParameters {
    pub message_group_id: Option<String>,
}

impl From<SerializableSqsParameters> for SqsParameters {
    fn from(parameters: SerializableSqsParameters) -> Self {
        SqsParametersBuilder::default()
            .set_message_group_id(parameters.message_group_id)
            .build()
    }
}
impl From<SqsParameters> for SerializableSqsParameters {
    fn from(parameters: SqsParameters) -> Self {
        Self {
            message_group_id: parameters.message_group_id,
        }
    }
}
#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct SerializableSageMakerPipelineParameters {
    pub pipeline_parameter_list: Option<Vec<SerializableSageMakerPipelineParameter>>,
}
impl TryFrom<SerializableSageMakerPipelineParameters> for SageMakerPipelineParameters {
    type Error = anyhow::Error;
    fn try_from(parameters: SerializableSageMakerPipelineParameters) -> Result<Self, Self::Error> {
        Ok(SageMakerPipelineParametersBuilder::default()
            .set_pipeline_parameter_list(if let Some(list) = parameters.pipeline_parameter_list {
                let mut result = Vec::new();
                for element in list.into_iter().map(SageMakerPipelineParameter::try_from) {
                    result.push(element?);
                }
                Some(result)
            } else {
                None
            })
            .build())
    }
}
impl From<SageMakerPipelineParameters> for SerializableSageMakerPipelineParameters {
    fn from(parameters: SageMakerPipelineParameters) -> Self {
        Self {
            pipeline_parameter_list: parameters.pipeline_parameter_list.map(|list| {
                list.into_iter()
                    .map(SerializableSageMakerPipelineParameter::from)
                    .collect()
            }),
        }
    }
}
#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct SerializableSageMakerPipelineParameter {
    pub name: String,
    pub value: String,
}
impl TryFrom<SerializableSageMakerPipelineParameter> for SageMakerPipelineParameter {
    type Error = anyhow::Error;
    fn try_from(parameter: SerializableSageMakerPipelineParameter) -> Result<Self, Self::Error> {
        SageMakerPipelineParameterBuilder::default()
            .set_name(Some(parameter.name))
            .set_value(Some(parameter.value))
            .build()
            .context("Failed to build SageMakerPipelineParameter from SerializableSageMakerPipelineParameter")
    }
}
impl From<SageMakerPipelineParameter> for SerializableSageMakerPipelineParameter {
    fn from(parameter: SageMakerPipelineParameter) -> Self {
        Self {
            name: parameter.name,
            value: parameter.value,
        }
    }
}
#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct SerializableKinesisParameters {
    pub partition_key: String,
}
impl TryFrom<SerializableKinesisParameters> for KinesisParameters {
    type Error = anyhow::Error;
    fn try_from(parameters: SerializableKinesisParameters) -> Result<Self, Self::Error> {
        KinesisParametersBuilder::default()
            .set_partition_key(Some(parameters.partition_key))
            .build()
            .context("Failed to build KinesisParameters from SerializableKinesisParameters")
    }
}

impl From<KinesisParameters> for SerializableKinesisParameters {
    fn from(parameters: KinesisParameters) -> Self {
        Self {
            partition_key: parameters.partition_key,
        }
    }
}
#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct SerializableEventBridgeParameters {
    pub detail_type: String,
    pub source: String,
}
impl TryFrom<SerializableEventBridgeParameters> for EventBridgeParameters {
    type Error = anyhow::Error;
    fn try_from(parameters: SerializableEventBridgeParameters) -> Result<Self, Self::Error> {
        EventBridgeParametersBuilder::default()
            .set_detail_type(Some(parameters.detail_type))
            .set_source(Some(parameters.source))
            .build()
            .context("Failed to build EventBridgeParameters from SerializableEventBridgeParameters")
    }
}

impl From<EventBridgeParameters> for SerializableEventBridgeParameters {
    fn from(parameters: EventBridgeParameters) -> Self {
        Self {
            detail_type: parameters.detail_type,
            source: parameters.source,
        }
    }
}

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct SerializableEcsParameters {
    pub task_definition_arn: String,
    pub task_count: Option<i32>,
    pub launch_type: Option<SerializableLaunchType>,
    pub network_configuration: Option<SerializableNetworkConfiguration>,
    pub platform_version: Option<String>,
    pub group: Option<String>,
    pub capacity_provider_strategy: Option<Vec<SerializableCapacityProviderStrategyItem>>,
    pub enable_ecs_managed_tags: Option<bool>,
    pub enable_execute_command: Option<bool>,
    pub placement_constraints: Option<Vec<SerializablePlacementConstraint>>,
    pub placement_strategy: Option<Vec<SerializablePlacementStrategy>>,
    pub propagate_tags: Option<SerializablePropagateTags>,
    pub reference_id: Option<String>,
    pub tags: Option<Vec<HashMap<String, String>>>,
}
impl TryFrom<SerializableEcsParameters> for EcsParameters {
    type Error = anyhow::Error;
    fn try_from(parameters: SerializableEcsParameters) -> Result<Self, Self::Error> {
        EcsParametersBuilder::default()
            .set_task_definition_arn(Some(parameters.task_definition_arn))
            .set_task_count(parameters.task_count)
            .set_launch_type(parameters.launch_type.map(LaunchType::from))
            .set_network_configuration(
                if let Some(r) = parameters
                    .network_configuration
                    .map(NetworkConfiguration::try_from)
                {
                    Some(r?)
                } else {
                    None
                },
            )
            .set_platform_version(parameters.platform_version)
            .set_group(parameters.group)
            .set_capacity_provider_strategy(
                if let Some(strategy) = parameters.capacity_provider_strategy {
                    let mut result = Vec::new();
                    for element in strategy
                        .into_iter()
                        .map(CapacityProviderStrategyItem::try_from)
                    {
                        result.push(element?);
                    }
                    Some(result)
                } else {
                    None
                },
            )
            .set_enable_ecs_managed_tags(parameters.enable_ecs_managed_tags)
            .set_enable_execute_command(parameters.enable_execute_command)
            .set_placement_constraints(parameters.placement_constraints.map(|constraints| {
                constraints
                    .into_iter()
                    .map(PlacementConstraint::from)
                    .collect()
            }))
            .set_placement_strategy(
                parameters
                    .placement_strategy
                    .map(|strategy| strategy.into_iter().map(PlacementStrategy::from).collect()),
            )
            .set_propagate_tags(parameters.propagate_tags.map(PropagateTags::from))
            .set_reference_id(parameters.reference_id)
            .set_tags(parameters.tags)
            .build()
            .context("Failed to build EcsParameters from SerializableEcsParameters")
    }
}
impl From<EcsParameters> for SerializableEcsParameters {
    fn from(parameters: EcsParameters) -> Self {
        Self {
            task_definition_arn: parameters.task_definition_arn,
            task_count: parameters.task_count,
            launch_type: parameters.launch_type.map(SerializableLaunchType::from),
            network_configuration: parameters
                .network_configuration
                .map(SerializableNetworkConfiguration::from),
            platform_version: parameters.platform_version,
            group: parameters.group,
            capacity_provider_strategy: parameters.capacity_provider_strategy.map(|strategy| {
                strategy
                    .into_iter()
                    .map(SerializableCapacityProviderStrategyItem::from)
                    .collect()
            }),
            enable_ecs_managed_tags: parameters.enable_ecs_managed_tags,
            enable_execute_command: parameters.enable_execute_command,
            placement_constraints: parameters.placement_constraints.map(|constraints| {
                constraints
                    .into_iter()
                    .map(SerializablePlacementConstraint::from)
                    .collect()
            }),
            placement_strategy: parameters.placement_strategy.map(|strategy| {
                strategy
                    .into_iter()
                    .map(SerializablePlacementStrategy::from)
                    .collect()
            }),
            propagate_tags: parameters
                .propagate_tags
                .map(|propagate_tags| match propagate_tags {
                    aws_sdk_scheduler::types::PropagateTags::TaskDefinition => {
                        SerializablePropagateTags::TaskDefinition
                    }
                    _ => SerializablePropagateTags::Unknown,
                }),
            reference_id: parameters.reference_id,
            tags: parameters.tags,
        }
    }
}

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub enum SerializablePropagateTags {
    #[default]
    TaskDefinition,
    Unknown,
}
impl From<SerializablePropagateTags> for aws_sdk_scheduler::types::PropagateTags {
    fn from(propagate_tags: SerializablePropagateTags) -> Self {
        match propagate_tags {
            SerializablePropagateTags::TaskDefinition => {
                aws_sdk_scheduler::types::PropagateTags::TaskDefinition
            }
            _ => aws_sdk_scheduler::types::PropagateTags::TaskDefinition,
        }
    }
}
#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct SerializablePlacementStrategy {
    pub r#type: Option<SerializablePlacementStrategyType>,
    pub field: Option<String>,
}
impl From<SerializablePlacementStrategy> for PlacementStrategy {
    fn from(strategy: SerializablePlacementStrategy) -> Self {
        PlacementStrategyBuilder::default()
            .set_type(strategy.r#type.map(PlacementStrategyType::from))
            .set_field(strategy.field)
            .build()
    }
}

impl From<PlacementStrategy> for SerializablePlacementStrategy {
    fn from(strategy: PlacementStrategy) -> Self {
        Self {
            r#type: strategy.r#type.map(SerializablePlacementStrategyType::from),
            field: strategy.field,
        }
    }
}

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub enum SerializablePlacementStrategyType {
    #[default]
    Binpack,
    Random,
    Spread,
    Unknown,
}

impl From<SerializablePlacementStrategyType> for PlacementStrategyType {
    fn from(strategy_type: SerializablePlacementStrategyType) -> Self {
        match strategy_type {
            SerializablePlacementStrategyType::Binpack => PlacementStrategyType::Binpack,
            SerializablePlacementStrategyType::Random => PlacementStrategyType::Random,
            SerializablePlacementStrategyType::Spread => PlacementStrategyType::Spread,
            _ => PlacementStrategyType::Binpack,
        }
    }
}
impl From<PlacementStrategyType> for SerializablePlacementStrategyType {
    fn from(strategy_type: PlacementStrategyType) -> Self {
        match strategy_type {
            PlacementStrategyType::Binpack => Self::Binpack,
            PlacementStrategyType::Random => Self::Random,
            PlacementStrategyType::Spread => Self::Spread,
            _ => Self::Unknown,
        }
    }
}

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct SerializablePlacementConstraint {
    pub r#type: Option<SerializablePlacementConstraintType>,
    pub expression: Option<String>,
}
impl From<SerializablePlacementConstraint> for PlacementConstraint {
    fn from(constraint: SerializablePlacementConstraint) -> Self {
        PlacementConstraintBuilder::default()
            .set_type(constraint.r#type.map(PlacementConstraintType::from))
            .set_expression(constraint.expression)
            .build()
    }
}
impl From<PlacementConstraint> for SerializablePlacementConstraint {
    fn from(constraint: PlacementConstraint) -> Self {
        Self {
            r#type: constraint
                .r#type
                .map(SerializablePlacementConstraintType::from),
            expression: constraint.expression,
        }
    }
}
#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub enum SerializablePlacementConstraintType {
    #[default]
    DistinctInstance,
    MemberOf,
    Unknown,
}
impl From<SerializablePlacementConstraintType> for PlacementConstraintType {
    fn from(constraint_type: SerializablePlacementConstraintType) -> Self {
        match constraint_type {
            SerializablePlacementConstraintType::DistinctInstance => {
                PlacementConstraintType::DistinctInstance
            }
            SerializablePlacementConstraintType::MemberOf => PlacementConstraintType::MemberOf,
            _ => PlacementConstraintType::DistinctInstance,
        }
    }
}
impl From<PlacementConstraintType> for SerializablePlacementConstraintType {
    fn from(constraint_type: PlacementConstraintType) -> Self {
        match constraint_type {
            PlacementConstraintType::DistinctInstance => Self::DistinctInstance,
            PlacementConstraintType::MemberOf => Self::MemberOf,
            _ => Self::Unknown,
        }
    }
}

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct SerializableCapacityProviderStrategyItem {
    pub capacity_provider: String,
    pub weight: i32,
    pub base: i32,
}
impl TryFrom<SerializableCapacityProviderStrategyItem> for CapacityProviderStrategyItem {
    type Error = anyhow::Error;
    fn try_from(item: SerializableCapacityProviderStrategyItem) -> Result<Self, Self::Error> {
        CapacityProviderStrategyItemBuilder::default()
            .set_capacity_provider(Some(item.capacity_provider))
            .set_weight(Some(item.weight))
            .set_base(Some(item.base))
            .build()
            .context("Failed to build CapacityProviderStrategyItem from SerializableCapacityProviderStrategyItem")
    }
}
impl From<CapacityProviderStrategyItem> for SerializableCapacityProviderStrategyItem {
    fn from(item: CapacityProviderStrategyItem) -> Self {
        Self {
            capacity_provider: item.capacity_provider,
            weight: item.weight,
            base: item.base,
        }
    }
}
#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct SerializableNetworkConfiguration {
    pub awsvpc_configuration: Option<SerializableAwsVpcConfiguration>,
}
impl TryFrom<SerializableNetworkConfiguration> for NetworkConfiguration {
    type Error = anyhow::Error;
    fn try_from(configuration: SerializableNetworkConfiguration) -> Result<Self, Self::Error> {
        Ok(NetworkConfigurationBuilder::default()
            .set_awsvpc_configuration(
                if let Some(r) = configuration
                    .awsvpc_configuration
                    .map(AwsVpcConfiguration::try_from)
                {
                    Some(r?)
                } else {
                    None
                },
            )
            .build())
    }
}
impl From<NetworkConfiguration> for SerializableNetworkConfiguration {
    fn from(configuration: NetworkConfiguration) -> Self {
        Self {
            awsvpc_configuration: configuration
                .awsvpc_configuration
                .map(SerializableAwsVpcConfiguration::from),
        }
    }
}

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct SerializableAwsVpcConfiguration {
    pub subnets: Vec<String>,
    pub security_groups: Option<Vec<String>>,
    pub assign_public_ip: Option<SerializableAssignPublicIp>,
}
impl TryFrom<SerializableAwsVpcConfiguration> for AwsVpcConfiguration {
    type Error = anyhow::Error;
    fn try_from(configuration: SerializableAwsVpcConfiguration) -> Result<Self, Self::Error> {
        AwsVpcConfigurationBuilder::default()
            .set_subnets(Some(configuration.subnets))
            .set_security_groups(configuration.security_groups)
            .set_assign_public_ip(configuration.assign_public_ip.map(AssignPublicIp::from))
            .build()
            .context("Failed to build AwsVpcConfiguration from SerializableAwsVpcConfiguration")
    }
}

impl From<AwsVpcConfiguration> for SerializableAwsVpcConfiguration {
    fn from(configuration: AwsVpcConfiguration) -> Self {
        Self {
            subnets: configuration.subnets,
            security_groups: configuration.security_groups,
            assign_public_ip: configuration
                .assign_public_ip
                .map(SerializableAssignPublicIp::from),
        }
    }
}

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub enum SerializableAssignPublicIp {
    #[default]
    Disabled,
    Enabled,
    Unknown,
}

impl From<SerializableAssignPublicIp> for AssignPublicIp {
    fn from(assign_public_ip: SerializableAssignPublicIp) -> Self {
        match assign_public_ip {
            SerializableAssignPublicIp::Disabled => AssignPublicIp::Disabled,
            SerializableAssignPublicIp::Enabled => AssignPublicIp::Enabled,
            _ => AssignPublicIp::Disabled,
        }
    }
}
impl From<AssignPublicIp> for SerializableAssignPublicIp {
    fn from(assign_public_ip: AssignPublicIp) -> Self {
        match assign_public_ip {
            AssignPublicIp::Disabled => Self::Disabled,
            AssignPublicIp::Enabled => Self::Enabled,
            _ => Self::Unknown,
        }
    }
}

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub enum SerializableLaunchType {
    #[default]
    Ec2,
    External,
    Fargate,
    Unknown,
}
impl From<SerializableLaunchType> for LaunchType {
    fn from(launch_type: SerializableLaunchType) -> Self {
        match launch_type {
            SerializableLaunchType::Ec2 => LaunchType::Ec2,
            SerializableLaunchType::External => LaunchType::External,
            SerializableLaunchType::Fargate => LaunchType::Fargate,
            _ => LaunchType::Ec2,
        }
    }
}
impl From<LaunchType> for SerializableLaunchType {
    fn from(launch_type: LaunchType) -> Self {
        match launch_type {
            LaunchType::Ec2 => Self::Ec2,
            LaunchType::External => Self::External,
            LaunchType::Fargate => Self::Fargate,
            _ => Self::Unknown,
        }
    }
}
#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct SerializableRetryPolicy {
    pub maximum_event_age_in_seconds: Option<i32>,
    pub maximum_retry_attempts: Option<i32>,
}
impl From<SerializableRetryPolicy> for RetryPolicy {
    fn from(policy: SerializableRetryPolicy) -> Self {
        RetryPolicyBuilder::default()
            .set_maximum_event_age_in_seconds(policy.maximum_event_age_in_seconds)
            .set_maximum_retry_attempts(policy.maximum_retry_attempts)
            .build()
    }
}
impl From<RetryPolicy> for SerializableRetryPolicy {
    fn from(policy: RetryPolicy) -> Self {
        Self {
            maximum_event_age_in_seconds: policy.maximum_event_age_in_seconds,
            maximum_retry_attempts: policy.maximum_retry_attempts,
        }
    }
}

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct SerializableSDeadLetterConfig {
    pub arn: Option<String>,
}
impl From<SerializableSDeadLetterConfig> for DeadLetterConfig {
    fn from(config: SerializableSDeadLetterConfig) -> Self {
        DeadLetterConfigBuilder::default()
            .set_arn(config.arn)
            .build()
    }
}
impl From<DeadLetterConfig> for SerializableSDeadLetterConfig {
    fn from(config: DeadLetterConfig) -> Self {
        Self { arn: config.arn }
    }
}
