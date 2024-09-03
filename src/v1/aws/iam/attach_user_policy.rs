use aws_sdk_iam::{
    operation::{
        attach_user_policy::builders::AttachUserPolicyFluentBuilder,
        get_user_policy::GetUserPolicyOutput,
    },
    types::AttachedPolicy,
    Client,
};
use serde::{Deserialize, Serialize};

use crate::{
    prelude::{AwsManager, AwsResource, AwsResourceCreator, ResourceError},
    v1::manager::{ManagerError, ResourceManager},
};

use super::{policy::Policy, user::IAMUser};

pub type AttachUserPolicyOutput = SerializableGetUserPolicyOutput;
pub type AttachUserPolicyInput = SerializableAttachUserPolicyInput;
pub type AttachUserPolicyManager =
    AwsManager<AttachUserPolicyInput, AttachUserPolicyOutput, Client>;
pub type AttachUserPolicy<'a> = AwsResource<'a, AttachUserPolicyInput, AttachUserPolicyOutput>;

impl AwsResourceCreator for AttachUserPolicy<'_> {
    type Input = AttachUserPolicyInput;
    type Output = AttachUserPolicyOutput;
    fn r#type() -> crate::prelude::AwsType {
        crate::prelude::AwsType::AttachUserPolicy
    }
    fn manager(
        handle: &tokio::runtime::Handle,
        config: &aws_config::SdkConfig,
    ) -> std::sync::Arc<dyn ResourceManager<Self::Input, Self::Output>> {
        AttachUserPolicyManager::new(handle, Client::new(config), config).arc()
    }
    fn input_hook(_id: &str, _input: &mut Self::Input) {}
}

impl<'a> AttachUserPolicy<'a> {
    pub fn bind_user(self, user: &IAMUser) -> Result<AttachUserPolicy<'a>, ResourceError> {
        self.bind(user, move |this, other| {
            this.user_name = other.user.as_ref().map(|g| g.user_name.clone())
        })?;
        Ok(self)
    }
    pub fn bind_policy(self, policy: &Policy) -> Result<AttachUserPolicy<'a>, ResourceError> {
        self.bind(policy, move |this, other| {
            this.policy_arn.clone_from(&other.arn)
        })?;
        Ok(self)
    }
}

impl AttachUserPolicyManager {
    fn lookup(
        &self,
        user_name: &str,
        policy_arn: &str,
    ) -> Result<Option<AttachUserPolicyOutput>, ManagerError> {
        self.handle.block_on(async {
            self.client
                .list_attached_user_policies()
                .user_name(user_name)
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
                        Some(SerializableGetUserPolicyOutput {
                            user_name: user_name.to_string(),
                            policy_name: filtered[0].policy_arn().unwrap().to_string(),
                            policy_document: Default::default(),
                        })
                    }
                })
                .or_else(|e| match e {
                    ManagerError::LookupFail(ref msg) if msg.contains("NoSuchEntity") => {
                        println!("UserPolicy not found [{}] [{}]", user_name, policy_arn);
                        Ok(None)
                    }
                    _ => Err(e),
                })
        })
    }
    fn create(
        &self,
        input: &mut AttachUserPolicyInput,
    ) -> Result<AttachUserPolicyOutput, crate::v1::manager::ManagerError> {
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
                "User has been attached but not retrieved".to_string(),
            ))
        }
    }
    fn delete(&self, user_name: &str, policy_name: &str) -> Result<bool, ManagerError> {
        self.handle.block_on(async {
            self.client
                .detach_user_policy()
                .user_name(user_name)
                .policy_arn(policy_name)
                .send()
                .await
                .map_err(|e| ManagerError::DeleteFail(format!("{:?}", e.into_source())))
                .map(|_| true)
        })
    }
}

impl ResourceManager<AttachUserPolicyInput, AttachUserPolicyOutput> for AttachUserPolicyManager {
    fn lookup(
        &self,
        latest: &AttachUserPolicyOutput,
    ) -> Result<Option<AttachUserPolicyOutput>, crate::v1::manager::ManagerError> {
        self.lookup(&latest.user_name, &latest.policy_name)
    }

    fn lookup_by_input(
        &self,
        input: &AttachUserPolicyInput,
    ) -> Result<Option<AttachUserPolicyOutput>, crate::v1::manager::ManagerError> {
        if input.user_name.is_none() || input.policy_arn.is_none() {
            Err(ManagerError::LookupFail(
                "Expect input to have user_name and policy_arn".to_string(),
            ))
        } else {
            let user_name = input.user_name.as_ref().unwrap();
            let policy_arn = input.policy_arn.as_ref().unwrap();
            self.lookup(user_name, policy_arn)
        }
    }

    fn create(
        &self,
        input: &mut AttachUserPolicyInput,
    ) -> Result<AttachUserPolicyOutput, crate::v1::manager::ManagerError> {
        self.create(input)
    }

    fn delete(
        &self,
        latest: &AttachUserPolicyOutput,
    ) -> Result<bool, crate::v1::manager::ManagerError> {
        self.delete(&latest.user_name, &latest.policy_name)
    }

    fn syncup(
        &self,
        _latest: &AttachUserPolicyOutput,
        _input: &mut AttachUserPolicyInput,
    ) -> Result<Option<AttachUserPolicyOutput>, crate::v1::manager::ManagerError> {
        Ok(None)
    }
}
#[derive(Default, Serialize, Deserialize, Debug, Clone)]

pub struct SerializableAttachUserPolicyInput {
    pub user_name: Option<String>,
    pub policy_arn: Option<String>,
}
impl SerializableAttachUserPolicyInput {
    pub fn to_aws_input(self, client: &Client) -> AttachUserPolicyFluentBuilder {
        client
            .attach_user_policy()
            .set_policy_arn(self.policy_arn)
            .set_user_name(self.user_name)
    }
}
#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct SerializableGetUserPolicyOutput {
    pub user_name: String,
    pub policy_name: String,
    pub policy_document: String,
}

impl From<GetUserPolicyOutput> for SerializableGetUserPolicyOutput {
    fn from(value: GetUserPolicyOutput) -> Self {
        Self {
            user_name: value.user_name,
            policy_name: value.policy_name,
            policy_document: value.policy_document,
        }
    }
}
