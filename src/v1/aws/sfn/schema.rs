use serde::{Deserialize, Serialize};

#[derive(Default, Clone, Serialize, Deserialize, Debug)]
pub struct StateMachineDocument {
    #[serde(rename = "Comment")]
    pub comment: Option<String>,
    #[serde(rename = "StartAt")]
    pub start_at: String,
    #[serde(rename = "States")]
    pub states: std::collections::HashMap<String, State>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(tag = "Type")]
pub enum State {
    Task(TaskState),
    Choice(ChoiceState),
    Wait(WaitState),
    Succeed(SucceedState),
    Fail(FailState),
    Parallel(ParallelState),
    Map(MapState),
    Pass(PassState),
}

#[derive(Default, Clone, Serialize, Deserialize, Debug)]
pub struct TaskState {
    #[serde(rename = "Resource")]
    pub resource: String,
    #[serde(rename = "Next")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next: Option<String>,
    #[serde(rename = "End")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end: Option<bool>,
    #[serde(rename = "InputPath")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_path: Option<String>,
    #[serde(rename = "OutputPath")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "ResultPath")]
    pub result_path: Option<String>,
    #[serde(rename = "Parameters")]
    pub parameters: Option<serde_json::Value>,
    #[serde(rename = "TimeoutSeconds")]
    #[serde(default = "default_timeout_seconds")]
    pub timeout_seconds: u64,
    #[serde(default = "default_heartbeat_seconds")]
    #[serde(rename = "HeartbeatSeconds")]
    pub heartbeat_seconds: u64,
    #[serde(rename = "Retry")]
    pub retry: Vec<RetryCatch>,
    #[serde(rename = "Catch")]
    pub catch: Vec<RetryCatch>,
}
pub fn default_heartbeat_seconds() -> u64 {
    99999999
}
pub fn default_timeout_seconds() -> u64 {
    99999999
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct RetryCatch {
    #[serde(rename = "ErrorEquals")]
    pub error_equals: Vec<String>,
    #[serde(rename = "IntervalSeconds")]
    pub interval_seconds: Option<u64>,
    #[serde(rename = "MaxAttempts")]
    pub max_attempts: Option<u64>,
    #[serde(rename = "BackoffRate")]
    pub backoff_rate: Option<f64>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ChoiceState {
    #[serde(rename = "Choices")]
    pub choices: Vec<ChoiceRule>,
    #[serde(rename = "Default")]
    pub default: Option<String>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ChoiceRule {
    #[serde(rename = "Variable")]
    pub variable: String,
    #[serde(flatten)]
    pub operator: ChoiceOperator,
    #[serde(rename = "Next")]
    pub next: String,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(tag = "Operator", rename_all = "camelCase")]
pub enum ChoiceOperator {
    StringEquals { string_value: String },
    NumericEquals { numeric_value: f64 },
    BooleanEquals { boolean_value: bool },
    // More operators can be added here
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct WaitState {
    #[serde(rename = "Seconds")]
    pub seconds: Option<u64>,
    #[serde(rename = "Timestamp")]
    pub timestamp: Option<String>,
    #[serde(rename = "Next")]
    pub next: Option<String>,
    #[serde(rename = "End")]
    pub end: Option<bool>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct SucceedState {
    #[serde(rename = "Comment")]
    pub comment: Option<String>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct FailState {
    #[serde(rename = "Cause")]
    pub cause: Option<String>,
    #[serde(rename = "Error")]
    pub error: Option<String>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ParallelState {
    #[serde(rename = "Branches")]
    pub branches: Vec<StateMachineDocument>,
    #[serde(rename = "Next")]
    pub next: Option<String>,
    #[serde(rename = "End")]
    pub end: Option<bool>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct MapState {
    #[serde(rename = "Iterator")]
    pub iterator: StateMachineDocument,
    #[serde(rename = "MaxConcurrency")]
    pub max_concurrency: Option<u32>,
    #[serde(rename = "Next")]
    pub next: Option<String>,
    #[serde(rename = "End")]
    pub end: Option<bool>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct PassState {
    #[serde(rename = "Next")]
    pub next: Option<String>,
    #[serde(rename = "End")]
    pub end: Option<bool>,
    #[serde(rename = "InputPath")]
    pub input_path: Option<String>,
    #[serde(rename = "OutputPath")]
    pub output_path: Option<String>,
    #[serde(rename = "ResultPath")]
    pub result_path: Option<String>,
    #[serde(rename = "Parameters")]
    pub parameters: Option<serde_json::Value>,
}
