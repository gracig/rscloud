use aws_sdk_iam::{operation::create_policy::builders::CreatePolicyFluentBuilder, types, Client};
use serde::{Deserialize, Serialize};

use crate::{
    prelude::{
        ArnProvider, AwsManager, AwsResource, AwsResourceCreator, ResourceError, SerializableTag,
    },
    v1::manager::{ManagerError, ResourceManager},
};

pub type PolicyInput = SerializableCreatePolicyInput;
pub type PolicyOutput = SerializablePolicy;
pub type PolicyManager = AwsManager<PolicyInput, PolicyOutput, Client>;
pub type Policy<'a> = AwsResource<'a, PolicyInput, PolicyOutput>;
impl AwsResourceCreator for Policy<'_> {
    type Input = PolicyInput;
    type Output = PolicyOutput;
    fn r#type() -> crate::prelude::AwsType {
        crate::prelude::AwsType::Policy
    }
    fn manager(
        handle: &tokio::runtime::Handle,
        config: &aws_config::SdkConfig,
    ) -> std::sync::Arc<dyn ResourceManager<Self::Input, Self::Output>> {
        PolicyManager::new(handle, Client::new(config), config).arc()
    }

    fn input_hook(id: &str, input: &mut Self::Input) {
        if input.policy_name.is_none() {
            input.policy_name = Some(id.to_string())
        }
    }
}

impl<'a> Policy<'a> {
    pub fn configure<Input: Clone + 'static, Output: Clone + ArnProvider + 'static>(
        &self,
        effect: &'static str,
        action: Vec<&'static str>,
        resource: &AwsResource<Input, Output>,
    ) -> Result<(), ResourceError> {
        self.bind(resource, move |this, other| {
            if let Some(arn) = other.arn() {
                this.policy_document = Some(
                    serde_json::json!({
                        "Version": "2012-10-17",
                        "Statement": [
                            {
                                "Effect": effect,
                                "Action": action,
                                "Resource": arn
                            }
                        ]
                    })
                    .to_string(),
                )
            }
        })
    }
}

impl PolicyManager {
    fn lookup(
        &self,
        policy_arn: &str,
    ) -> Result<Option<PolicyOutput>, crate::v1::manager::ManagerError> {
        self.handle.block_on(async {
            self.client
                .get_policy()
                .policy_arn(policy_arn)
                .send()
                .await
                .map_err(|e| ManagerError::LookupFail(format!("{:?}", e.into_source())))
                .map(|output| output.policy.map(PolicyOutput::from))
                .or_else(|e| match e {
                    ManagerError::LookupFail(ref msg) if msg.contains("NoSuchEntity") => {
                        println!("Policy Cannot be found");
                        Ok(None)
                    }
                    _ => {
                        println!("Policy Could not identifyt message");
                        Err(e)
                    }
                })
        })
    }
    fn lookup_by_input(
        &self,
        input: &PolicyInput,
    ) -> Result<Option<PolicyOutput>, crate::v1::manager::ManagerError> {
        let account_id = self.get_identity()?.account.unwrap_or_default();
        self.handle.block_on(async {
            let policy_arn = format!(
                "arn:aws:iam::{}:policy/{}",
                account_id,
                input.policy_name.as_ref().unwrap()
            );
            self.client
                .get_policy()
                .policy_arn(policy_arn)
                .send()
                .await
                .map_err(|e| ManagerError::LookupFail(format!("{:?}", e.into_source())))
                .map(|output| output.policy.map(PolicyOutput::from))
                .or_else(|e| match e {
                    ManagerError::LookupFail(ref msg) if msg.contains("NoSuchEntity") => {
                        println!("Policy Cannot be found");
                        Ok(None)
                    }
                    _ => {
                        println!("Policy Could not identifyt message");
                        Err(e)
                    }
                })
        })
    }
    fn create(
        &self,
        input: &mut PolicyInput,
    ) -> Result<PolicyOutput, crate::v1::manager::ManagerError> {
        let output = self.handle.block_on(async {
            input
                .clone()
                .to_aws_input(&self.client)
                .send()
                .await
                .map_err(|e| ManagerError::CreateFail(format!("{:?}", e.into_source())))
        })?;
        let arn = output.policy.and_then(|policy| policy.arn).ok_or_else(|| {
            ManagerError::CreateFail("Policy not presnet in the create response".to_string())
        })?;
        match self.lookup(&arn) {
            Ok(Some(policy)) => Ok(policy),
            Ok(None) => Err(ManagerError::CreateFail(
                "Policy create but not found".to_string(),
            )),
            Err(e) => Err(e),
        }
    }
    fn delete(&self, latest: &PolicyOutput) -> Result<bool, crate::v1::manager::ManagerError> {
        self.handle.block_on(async {
            self.client
                .delete_policy()
                .set_policy_arn(latest.arn.clone())
                .send()
                .await
                .map_err(|e| ManagerError::DeleteFail(format!("{:?}", e.into_source())))
                .map(|_| true)
        })
    }
    fn syncup(
        &self,
        _latest: &PolicyOutput,
        _input: &mut PolicyInput,
    ) -> Result<Option<PolicyOutput>, crate::v1::manager::ManagerError> {
        Ok(None)
    }
}

impl ResourceManager<PolicyInput, PolicyOutput> for PolicyManager {
    fn lookup(
        &self,
        latest: &PolicyOutput,
    ) -> Result<Option<PolicyOutput>, crate::v1::manager::ManagerError> {
        if let Some(arn) = latest.arn.as_ref() {
            self.lookup(arn)
        } else {
            Err(ManagerError::LookupFail(
                "Could not extract arn from latest".to_string(),
            ))
        }
    }

    fn lookup_by_input(
        &self,
        input: &PolicyInput,
    ) -> Result<Option<PolicyOutput>, crate::v1::manager::ManagerError> {
        self.lookup_by_input(input)
    }

    fn create(
        &self,
        input: &mut PolicyInput,
    ) -> Result<PolicyOutput, crate::v1::manager::ManagerError> {
        self.create(input)
    }

    fn delete(&self, latest: &PolicyOutput) -> Result<bool, crate::v1::manager::ManagerError> {
        self.delete(latest)
    }

    fn syncup(
        &self,
        latest: &PolicyOutput,
        input: &mut PolicyInput,
    ) -> Result<Option<PolicyOutput>, crate::v1::manager::ManagerError> {
        self.syncup(latest, input)
    }
}

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct SerializableCreatePolicyInput {
    pub policy_name: Option<String>,
    pub path: Option<String>,
    pub policy_document: Option<String>,
    pub description: Option<String>,
    pub tags: Option<Vec<SerializableTag>>,
}

impl SerializableCreatePolicyInput {
    pub fn to_aws_input(self, client: &Client) -> CreatePolicyFluentBuilder {
        client
            .create_policy()
            .set_description(self.description)
            .set_path(self.path)
            .set_policy_document(self.policy_document)
            .set_policy_name(self.policy_name)
    }
}

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct SerializablePolicy {
    pub policy_name: Option<String>,
    pub policy_id: Option<String>,
    pub arn: Option<String>,
    pub path: Option<String>,
    pub default_version_id: Option<String>,
    pub attachment_count: Option<i32>,
    pub permissions_boundary_usage_count: Option<i32>,
    pub is_attachable: bool,
    pub description: Option<String>,
    pub tags: Option<Vec<SerializableTag>>,
}

impl From<types::Policy> for SerializablePolicy {
    fn from(value: types::Policy) -> Self {
        Self {
            policy_name: value.policy_name,
            policy_id: value.policy_id,
            arn: value.arn,
            path: value.path,
            default_version_id: value.default_version_id,
            attachment_count: value.attachment_count,
            permissions_boundary_usage_count: value.permissions_boundary_usage_count,
            is_attachable: value.is_attachable,
            description: value.description,
            tags: value.tags.map(|v| {
                v.into_iter()
                    .map(|t| SerializableTag {
                        key: t.key,
                        value: t.value,
                    })
                    .collect()
            }),
        }
    }
}
