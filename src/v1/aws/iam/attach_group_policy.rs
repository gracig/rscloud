use aws_sdk_iam::{
    operation::{
        attach_group_policy::builders::AttachGroupPolicyFluentBuilder,
        get_group_policy::GetGroupPolicyOutput,
    },
    types::AttachedPolicy,
    Client,
};
use serde::{Deserialize, Serialize};

use crate::{
    prelude::{AwsManager, AwsResource, AwsResourceCreator, ResourceError},
    v1::manager::{ManagerError, ResourceManager},
};

use super::{group::IAMGroup, policy::Policy};

pub type AttachGroupPolicyOutput = SerializableGetGroupPolicyOutput;
pub type AttachGroupPolicyInput = SerializableAttachGroupPolicyInput;
pub type AttachGroupPolicyManager =
    AwsManager<AttachGroupPolicyInput, AttachGroupPolicyOutput, Client>;
pub type AttachGroupPolicy<'a> = AwsResource<'a, AttachGroupPolicyInput, AttachGroupPolicyOutput>;

impl AwsResourceCreator for AttachGroupPolicy<'_> {
    type Input = AttachGroupPolicyInput;
    type Output = AttachGroupPolicyOutput;
    fn r#type() -> crate::prelude::AwsType {
        crate::prelude::AwsType::AttachGroupPolicy
    }
    fn manager(
        handle: &tokio::runtime::Handle,
        config: &aws_config::SdkConfig,
    ) -> std::sync::Arc<dyn ResourceManager<Self::Input, Self::Output>> {
        AttachGroupPolicyManager::new(handle, Client::new(config), config).arc()
    }
    fn input_hook(_id: &str, _input: &mut Self::Input) {}
}

impl<'a> AttachGroupPolicy<'a> {
    pub fn bind_group(self, group: &IAMGroup) -> Result<AttachGroupPolicy<'a>, ResourceError> {
        self.bind(group, move |this, other| {
            this.group_name = other.group.as_ref().map(|g| g.group_name.clone())
        })?;
        Ok(self)
    }
    pub fn bind_policy(self, policy: &Policy) -> Result<AttachGroupPolicy<'a>, ResourceError> {
        self.bind(policy, move |this, other| {
            this.policy_arn.clone_from(&other.arn)
        })?;
        Ok(self)
    }
}

impl AttachGroupPolicyManager {
    fn lookup(
        &self,
        group_name: &str,
        policy_arn: &str,
    ) -> Result<Option<AttachGroupPolicyOutput>, ManagerError> {
        self.handle.block_on(async {
            self.client
                .list_attached_group_policies()
                .group_name(group_name)
                .send()
                .await
                .map_err(|e| ManagerError::LookupFail(format!("{:?}", e.into_source())))
                .map(|response| {
                    let filtered = response
                        .attached_policies()
                        .iter()
                        .filter(|&a| match a.policy_arn() {
                            Some(arn) => *arn == *policy_arn,
                            None => false,
                        })
                        .collect::<Vec<&AttachedPolicy>>();
                    if filtered.is_empty() {
                        None
                    } else {
                        Some(SerializableGetGroupPolicyOutput {
                            group_name: group_name.to_string(),
                            policy_name: filtered[0].policy_arn().unwrap().to_string(),
                            policy_document: Default::default(),
                        })
                    }
                })
                .or_else(|e| match e {
                    ManagerError::LookupFail(ref msg) if msg.contains("NoSuchEntity") => {
                        println!("GroupPolicy not found [{}] [{}]", group_name, policy_arn);
                        Ok(None)
                    }
                    _ => Err(e),
                })
        })
    }
    fn create(
        &self,
        input: &mut AttachGroupPolicyInput,
    ) -> Result<AttachGroupPolicyOutput, crate::v1::manager::ManagerError> {
        self.handle.block_on(async {
            input
                .clone()
                .to_aws_input(&self.client)
                .send()
                .await
                .map_err(|e| ManagerError::CreateFail(format!("{:?}", e.into_source())))
        })?;

        if let Some(output) = self.lookup_by_input(input)? {
            Ok(output)
        } else {
            Err(ManagerError::CreateFail(
                "Group has been attached but not retrieved".to_string(),
            ))
        }
    }
    fn delete(&self, group_name: &str, policy_name: &str) -> Result<bool, ManagerError> {
        self.handle.block_on(async {
            self.client
                .detach_group_policy()
                .group_name(group_name)
                .policy_arn(policy_name)
                .send()
                .await
                .map_err(|e| ManagerError::DeleteFail(format!("{:?}", e.into_source())))
                .map(|_| true)
        })
    }
}

impl ResourceManager<AttachGroupPolicyInput, AttachGroupPolicyOutput> for AttachGroupPolicyManager {
    fn lookup(
        &self,
        latest: &AttachGroupPolicyOutput,
    ) -> Result<Option<AttachGroupPolicyOutput>, crate::v1::manager::ManagerError> {
        self.lookup(&latest.group_name, &latest.policy_name)
    }

    fn lookup_by_input(
        &self,
        input: &AttachGroupPolicyInput,
    ) -> Result<Option<AttachGroupPolicyOutput>, crate::v1::manager::ManagerError> {
        if input.group_name.is_none() || input.policy_arn.is_none() {
            Err(ManagerError::LookupFail(
                "Expect input to have group_name and policy_arn".to_string(),
            ))
        } else {
            let group_name = input.group_name.as_ref().unwrap();
            let policy_arn = input.policy_arn.as_ref().unwrap();
            self.lookup(group_name, policy_arn)
        }
    }

    fn create(
        &self,
        input: &mut AttachGroupPolicyInput,
    ) -> Result<AttachGroupPolicyOutput, crate::v1::manager::ManagerError> {
        self.create(input)
    }

    fn delete(
        &self,
        latest: &AttachGroupPolicyOutput,
    ) -> Result<bool, crate::v1::manager::ManagerError> {
        self.delete(&latest.group_name, &latest.policy_name)
    }

    fn syncup(
        &self,
        _latest: &AttachGroupPolicyOutput,
        _input: &mut AttachGroupPolicyInput,
    ) -> Result<Option<AttachGroupPolicyOutput>, crate::v1::manager::ManagerError> {
        Ok(None)
    }
}
#[derive(Default, Serialize, Deserialize, Debug, Clone)]

pub struct SerializableAttachGroupPolicyInput {
    pub group_name: Option<String>,
    pub policy_arn: Option<String>,
}
impl SerializableAttachGroupPolicyInput {
    pub fn to_aws_input(self, client: &Client) -> AttachGroupPolicyFluentBuilder {
        client
            .attach_group_policy()
            .set_policy_arn(self.policy_arn)
            .set_group_name(self.group_name)
    }
}
#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct SerializableGetGroupPolicyOutput {
    pub group_name: String,
    pub policy_name: String,
    pub policy_document: String,
}

impl From<GetGroupPolicyOutput> for SerializableGetGroupPolicyOutput {
    fn from(value: GetGroupPolicyOutput) -> Self {
        Self {
            group_name: value.group_name,
            policy_name: value.policy_name,
            policy_document: value.policy_document,
        }
    }
}
