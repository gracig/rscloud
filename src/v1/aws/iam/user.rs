use crate::{
    prelude::{ArnProvider, AwsManager, AwsResource, AwsResourceCreator, SerializableTag},
    v1::{
        manager::{ManagerError, ResourceManager},
        plan::ResourceItem,
    },
};
use anyhow::Context;
use aws_sdk_iam::{operation::create_user::builders::CreateUserFluentBuilder, types::User};
use aws_sdk_iam::{operation::get_user::GetUserOutput, Client};
use serde::{Deserialize, Serialize};

use super::{
    attach_user_policy::{AttachUserPolicy, AttachUserPolicyInput},
    policy::Policy,
    role::SerializableAttachedPermissionsBoundary,
};

pub type UserInput = SerializableCreateUserInput;
pub type UserOutput = SerializableGetUserOutput;
pub type UserManager = AwsManager<UserInput, UserOutput, Client>;
pub type IAMUser<'a> = AwsResource<'a, UserInput, UserOutput>;

impl ArnProvider for UserOutput {
    fn arn(&self) -> Option<Vec<String>> {
        self.user.as_ref().map(|x| vec![x.arn.clone()])
    }
}

impl<'a> IAMUser<'a> {
    pub fn attach_policy(&self, policy: &Policy) -> anyhow::Result<&IAMUser<'a>> {
        let policy_name = policy.inner.name();
        let user_name = self.inner.name();
        self.aws
            .resource::<AttachUserPolicy>(
                &format!("attach-{}-{}", policy_name, user_name),
                self.inner.state()?,
                Default::default(),
            )?
            .bind_policy(policy)?
            .bind_user(self)?;
        Ok(self)
    }
    pub fn attach_managed_policy(&self, policy_arn: &str) -> anyhow::Result<&IAMUser<'a>> {
        let policy_name = policy_arn.split('/').last().unwrap();
        let user_name = self.inner.name();
        self.aws
            .resource::<AttachUserPolicy>(
                &format!("attach-{}-{}", policy_name, user_name),
                self.inner.state()?,
                AttachUserPolicyInput {
                    policy_arn: Some(policy_arn.to_string()),
                    ..Default::default()
                },
            )?
            .bind_user(self)?;
        Ok(self)
    }
}

impl AwsResourceCreator for IAMUser<'_> {
    type Input = UserInput;
    type Output = UserOutput;
    fn r#type() -> crate::prelude::AwsType {
        crate::prelude::AwsType::User
    }
    fn manager(
        handle: &tokio::runtime::Handle,
        config: &aws_config::SdkConfig,
    ) -> std::sync::Arc<dyn ResourceManager<Self::Input, Self::Output>> {
        UserManager::new(handle, Client::new(config), config).arc()
    }
    fn input_hook(_id: &str, _input: &mut Self::Input) {}
}
impl UserManager {
    fn lookup(
        &self,
        user_name: &str,
    ) -> Result<Option<UserOutput>, crate::v1::manager::ManagerError> {
        self.handle.block_on(async {
            self.client
                .get_user()
                .user_name(user_name)
                .send()
                .await
                .map(UserOutput::from)
                .map(Some)
                .map_err(|e| ManagerError::LookupFail(format!("{:?}", e.into_source())))
                .or_else(|e| match e {
                    ManagerError::LookupFail(ref msg) if msg.contains("NoSuchEntity") => {
                        println!("User Cannot be found");
                        Ok(None)
                    }
                    _ => Err(e),
                })
        })
    }
    fn create(
        &self,
        input: &mut UserInput,
    ) -> Result<UserOutput, crate::v1::manager::ManagerError> {
        self.handle.block_on(async {
            input
                .clone()
                .to_aws_input(&self.client)
                .map_err(|e| ManagerError::CreateFail(format!("{:?}", e)))?
                .send()
                .await
                .map_err(|e| ManagerError::CreateFail(format!("{:?}", e.into_source())))
        })?;
        if let Some(user) = self.lookup_by_input(input)? {
            Ok(user)
        } else {
            Err(ManagerError::CreateFail("User not found".to_string()))
        }
    }
    fn delete(&self, user_name: &str) -> Result<bool, crate::v1::manager::ManagerError> {
        self.handle.block_on(async {
            self.client
                .delete_user()
                .user_name(user_name)
                .send()
                .await
                .map_err(|e| ManagerError::DeleteFail(format!("{:?}", e.into_source())))
                .map(|_| true)
        })
    }
}

impl ResourceManager<UserInput, UserOutput> for UserManager {
    fn lookup(
        &self,
        latest: &UserOutput,
    ) -> Result<Option<UserOutput>, crate::v1::manager::ManagerError> {
        let user_name = latest
            .user
            .as_ref()
            .map(|x| x.user_name.as_str())
            .unwrap_or("");
        self.lookup(user_name)
    }

    fn lookup_by_input(
        &self,
        input: &UserInput,
    ) -> Result<Option<UserOutput>, crate::v1::manager::ManagerError> {
        let user_name = input.user_name.as_deref().unwrap_or("");
        self.lookup(user_name)
    }

    fn create(
        &self,
        input: &mut UserInput,
    ) -> Result<UserOutput, crate::v1::manager::ManagerError> {
        self.create(input)
    }

    fn delete(&self, latest: &UserOutput) -> Result<bool, crate::v1::manager::ManagerError> {
        let user_name = latest
            .user
            .as_ref()
            .map(|x| x.user_name.as_str())
            .unwrap_or("");
        self.delete(user_name)
    }

    fn syncup(
        &self,
        _latest: &UserOutput,
        _input: &mut UserInput,
    ) -> Result<Option<UserOutput>, crate::v1::manager::ManagerError> {
        Ok(None)
    }
}

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct SerializableCreateUserInput {
    pub path: Option<String>,
    pub user_name: Option<String>,
    pub permissions_boundary: Option<String>,
    pub tags: Option<Vec<SerializableTag>>,
}

impl SerializableCreateUserInput {
    pub fn to_aws_input(self, client: &Client) -> anyhow::Result<CreateUserFluentBuilder> {
        Ok(client
            .create_user()
            .set_user_name(self.user_name)
            .set_path(self.path)
            .set_permissions_boundary(self.permissions_boundary)
            .set_tags(if let Some(tags) = self.tags {
                let mut result = vec![];
                for tag in tags {
                    result.push(
                        aws_sdk_iam::types::builders::TagBuilder::default()
                            .key(tag.key)
                            .value(tag.value)
                            .build()
                            .context("Could not build tag")?,
                    );
                }
                Some(result)
            } else {
                None
            }))
    }
}

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct SerializableGetUserOutput {
    pub user: Option<SerializableUser>,
}

impl From<GetUserOutput> for SerializableGetUserOutput {
    fn from(value: GetUserOutput) -> Self {
        Self {
            user: value.user.map(|x| x.into()),
        }
    }
}

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct SerializableUser {
    pub path: String,
    pub user_name: String,
    pub user_id: String,
    pub arn: String,
    pub permissions_boundary: Option<SerializableAttachedPermissionsBoundary>,
    pub tags: Option<Vec<SerializableTag>>,
}

impl From<User> for SerializableUser {
    fn from(value: User) -> Self {
        Self {
            path: value.path,
            user_name: value.user_name,
            user_id: value.user_id,
            arn: value.arn,
            permissions_boundary: value.permissions_boundary.map(|x| x.into()),
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
