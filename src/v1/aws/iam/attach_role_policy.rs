use aws_sdk_iam::{
    operation::{
        attach_role_policy::builders::AttachRolePolicyFluentBuilder,
        get_role_policy::GetRolePolicyOutput,
    },
    types::AttachedPolicy,
    Client,
};
use serde::{Deserialize, Serialize};

use crate::{
    prelude::{AwsManager, AwsResource, AwsResourceCreator, ResourceError},
    v1::manager::{ManagerError, ResourceManager},
};

use super::{policy::Policy, role::Role};

pub type AttachRolePolicyOutput = SerializableGetRolePolicyOutput;
pub type AttachRolePolicyInput = SerializableAttachRolePolicyInput;
pub type AttachRolePolicyManager =
    AwsManager<AttachRolePolicyInput, AttachRolePolicyOutput, Client>;
pub type AttachRolePolicy<'a> = AwsResource<'a, AttachRolePolicyInput, AttachRolePolicyOutput>;

impl AwsResourceCreator for AttachRolePolicy<'_> {
    type Input = AttachRolePolicyInput;
    type Output = AttachRolePolicyOutput;
    fn r#type() -> crate::prelude::AwsType {
        crate::prelude::AwsType::AttachRolePolicy
    }
    fn manager(
        handle: &tokio::runtime::Handle,
        config: &aws_config::SdkConfig,
    ) -> std::sync::Arc<dyn ResourceManager<Self::Input, Self::Output>> {
        AttachRolePolicyManager::new(handle, Client::new(config), config).arc()
    }
    fn input_hook(_id: &str, _input: &mut Self::Input) {}
}

impl<'a> AttachRolePolicy<'a> {
    pub fn bind_role(self, role: &Role) -> Result<AttachRolePolicy<'a>, ResourceError> {
        self.bind(role, move |this, other| {
            this.role_name = Some(other.role_name.clone())
        })?;
        Ok(self)
    }
    pub fn bind_policy(self, policy: &Policy) -> Result<AttachRolePolicy<'a>, ResourceError> {
        self.bind(policy, move |this, other| {
            this.policy_arn.clone_from(&other.arn)
        })?;
        Ok(self)
    }
}

impl AttachRolePolicyManager {
    fn lookup(
        &self,
        role_name: &str,
        policy_arn: &str,
    ) -> Result<Option<AttachRolePolicyOutput>, ManagerError> {
        self.handle.block_on(async {
            self.client
                .list_attached_role_policies()
                .role_name(role_name)
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
                        Some(SerializableGetRolePolicyOutput {
                            role_name: role_name.to_string(),
                            policy_name: filtered[0].policy_arn().unwrap().to_string(),
                            policy_document: Default::default(),
                        })
                    }
                })
                .or_else(|e| match e {
                    ManagerError::LookupFail(ref msg) if msg.contains("NoSuchEntity") => {
                        println!("RolePolicy not found [{}] [{}]", role_name, policy_arn);
                        Ok(None)
                    }
                    _ => Err(e),
                })
        })
    }
    fn create(
        &self,
        input: &mut AttachRolePolicyInput,
    ) -> Result<AttachRolePolicyOutput, crate::v1::manager::ManagerError> {
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
                "Role has been attached but not retrieved".to_string(),
            ))
        }
    }
    fn delete(&self, role_name: &str, policy_name: &str) -> Result<bool, ManagerError> {
        self.handle.block_on(async {
            self.client
                .detach_role_policy()
                .role_name(role_name)
                .policy_arn(policy_name)
                .send()
                .await
                .map_err(|e| ManagerError::DeleteFail(format!("{:?}", e.into_source())))
                .map(|_| true)
        })
    }
}

impl ResourceManager<AttachRolePolicyInput, AttachRolePolicyOutput> for AttachRolePolicyManager {
    fn lookup(
        &self,
        latest: &AttachRolePolicyOutput,
    ) -> Result<Option<AttachRolePolicyOutput>, crate::v1::manager::ManagerError> {
        self.lookup(&latest.role_name, &latest.policy_name)
    }

    fn lookup_by_input(
        &self,
        input: &AttachRolePolicyInput,
    ) -> Result<Option<AttachRolePolicyOutput>, crate::v1::manager::ManagerError> {
        if input.role_name.is_none() || input.policy_arn.is_none() {
            Err(ManagerError::LookupFail(
                "Expect input to have role_name and policy_arn".to_string(),
            ))
        } else {
            let role_name = input.role_name.as_ref().unwrap();
            let policy_arn = input.policy_arn.as_ref().unwrap();
            self.lookup(role_name, policy_arn)
        }
    }

    fn create(
        &self,
        input: &mut AttachRolePolicyInput,
    ) -> Result<AttachRolePolicyOutput, crate::v1::manager::ManagerError> {
        self.create(input)
    }

    fn delete(
        &self,
        latest: &AttachRolePolicyOutput,
    ) -> Result<bool, crate::v1::manager::ManagerError> {
        self.delete(&latest.role_name, &latest.policy_name)
    }

    fn syncup(
        &self,
        _latest: &AttachRolePolicyOutput,
        _input: &mut AttachRolePolicyInput,
    ) -> Result<Option<AttachRolePolicyOutput>, crate::v1::manager::ManagerError> {
        Ok(None)
    }
}
#[derive(Default, Serialize, Deserialize, Debug, Clone)]

pub struct SerializableAttachRolePolicyInput {
    pub role_name: Option<String>,
    pub policy_arn: Option<String>,
}
impl SerializableAttachRolePolicyInput {
    pub fn to_aws_input(self, client: &Client) -> AttachRolePolicyFluentBuilder {
        client
            .attach_role_policy()
            .set_policy_arn(self.policy_arn)
            .set_role_name(self.role_name)
    }
}
#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct SerializableGetRolePolicyOutput {
    pub role_name: String,
    pub policy_name: String,
    pub policy_document: String,
}

impl From<GetRolePolicyOutput> for SerializableGetRolePolicyOutput {
    fn from(value: GetRolePolicyOutput) -> Self {
        Self {
            role_name: value.role_name,
            policy_name: value.policy_name,
            policy_document: value.policy_document,
        }
    }
}
