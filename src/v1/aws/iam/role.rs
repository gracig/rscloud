use std::borrow::BorrowMut;

use crate::{
    prelude::{ArnProvider, AwsManager, AwsResource, AwsResourceCreator, SerializableTag},
    v1::{
        manager::{ManagerError, ResourceManager},
        plan::ResourceItem,
    },
};
use anyhow::{anyhow, Context};
use aws_sdk_iam::{
    operation::create_role::builders::CreateRoleFluentBuilder,
    types::{
        self, builders::TagBuilder, AttachedPermissionsBoundary, PermissionsBoundaryAttachmentType,
        RoleLastUsed, Tag,
    },
    Client,
};
use serde::{Deserialize, Serialize};

use super::{
    attach_role_policy::{AttachRolePolicy, AttachRolePolicyInput},
    policy::Policy,
};
pub type RoleInput = SerializableCreateRoleInput;
pub type RoleOutput = SerializableRole;
pub type RoleManager = AwsManager<RoleInput, RoleOutput, Client>;
pub type Role<'a> = AwsResource<'a, RoleInput, RoleOutput>;

impl ArnProvider for RoleOutput {
    fn arn(&self) -> Option<Vec<String>> {
        Some(vec![self.arn.clone()])
    }
}

impl AwsResourceCreator for Role<'_> {
    type Input = RoleInput;
    type Output = RoleOutput;
    fn r#type() -> crate::prelude::AwsType {
        crate::prelude::AwsType::Role
    }
    fn manager(
        handle: &tokio::runtime::Handle,
        config: &aws_config::SdkConfig,
    ) -> std::sync::Arc<dyn ResourceManager<Self::Input, Self::Output>> {
        RoleManager::new(handle, Client::new(config), config).arc()
    }
    fn input_hook(id: &str, input: &mut Self::Input) {
        if input.role_name.is_none() {
            input.role_name = Some(id.to_string())
        }
    }
}

impl<'a> Role<'a> {
    pub fn attach_policy(&self, policy: &Policy) -> anyhow::Result<&Role<'a>> {
        let policy_name = policy.inner.name();
        let role_name = self.inner.name();
        self.aws
            .resource::<AttachRolePolicy>(
                &format!("attach-{}-{}", policy_name, role_name),
                self.inner.state()?,
                Default::default(),
            )?
            .bind_policy(policy)?
            .bind_role(self)?;
        Ok(self)
    }
    pub fn attach_managed_policy(&self, policy_arn: &str) -> anyhow::Result<&Role<'a>> {
        let policy_name = policy_arn.split('/').last().unwrap();
        let role_name = self.inner.name();
        self.aws
            .resource::<AttachRolePolicy>(
                &format!("attach-{}-{}", policy_name, role_name),
                self.inner.state()?,
                AttachRolePolicyInput {
                    policy_arn: Some(policy_arn.to_string()),
                    ..Default::default()
                },
            )?
            .bind_role(self)?;
        Ok(self)
    }

    pub fn for_service(self, service: &str) -> anyhow::Result<Role<'a>> {
        match self.inner.resource.lock().borrow_mut() {
            Ok(resource) => {
                resource.input.assume_role_policy_document = Some(
                    serde_json::json!({
                        "Version": "2012-10-17",
                        "Statement": [
                            {
                                "Effect": "Allow",
                                "Principal": {
                                    "Service": service
                                },
                                "Action": "sts:AssumeRole"
                            }
                        ]
                    })
                    .to_string(),
                );
            }
            Err(_) => return Err(anyhow!("Could not lock resource")),
        };
        Ok(self)
    }
}

impl ResourceManager<RoleInput, RoleOutput> for RoleManager {
    fn lookup(&self, latest: &RoleOutput) -> Result<Option<RoleOutput>, ManagerError> {
        self.lookup(&latest.role_name)
    }
    fn create(&self, input: &mut RoleInput) -> Result<RoleOutput, ManagerError> {
        self.create(input)
    }
    fn delete(&self, latest: &RoleOutput) -> Result<bool, ManagerError> {
        self.delete(&latest.role_name)
    }
    fn syncup(
        &self,
        _latest: &RoleOutput,
        _input: &mut RoleInput,
    ) -> Result<Option<RoleOutput>, ManagerError> {
        Ok(None)
    }
    fn lookup_by_input(&self, input: &RoleInput) -> Result<Option<RoleOutput>, ManagerError> {
        if let Some(role_name) = input.role_name.as_ref() {
            self.lookup(role_name)
        } else {
            Err(ManagerError::CreateFail(format!(
                "role_name is required: {:?}",
                input
            )))
        }
    }
}

impl RoleManager {
    fn lookup(&self, role_name: &str) -> Result<Option<RoleOutput>, ManagerError> {
        self.handle.block_on(async {
            self.client
                .get_role()
                .role_name(role_name)
                .send()
                .await
                .map_err(|e| ManagerError::LookupFail(format!("{:?}", e.into_source())))
                .map(|r| r.role.map(SerializableRole::from))
                .or_else(|e| match e {
                    ManagerError::LookupFail(ref msg) if msg.contains("NoSuchEntity") => {
                        println!("Role Cannot be found");
                        Ok(None)
                    }
                    _ => {
                        println!("Role Could not identifyt message");
                        Err(e)
                    }
                })
        })
    }
    fn create(&self, input: &RoleInput) -> Result<RoleOutput, ManagerError> {
        self.handle.block_on(async {
            input
                .clone()
                .to_aws_input(&self.client)
                .map_err(|e| ManagerError::CreateFail(format!("{:?}", e)))?
                .send()
                .await
                .map_err(|e| ManagerError::CreateFail(format!("{:?}", e.into_source())))
                .map(|output| output.role)?
                .map(SerializableRole::from)
                .ok_or_else(|| ManagerError::CreateFail("Could not create role".to_string()))
        })
    }
    fn delete(&self, role_name: &str) -> Result<bool, ManagerError> {
        self.handle.block_on(async {
            self.client
                .delete_role()
                .role_name(role_name)
                .send()
                .await
                .map_err(|e| ManagerError::DeleteFail(format!("{:?}", e.into_source())))
                .map(|_| true)
        })
    }
}

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct SerializableRole {
    pub path: String,
    pub role_name: String,
    pub role_id: String,
    pub arn: String,
    pub create_date: i64,
    pub assume_role_policy_document: Option<String>,
    pub description: Option<String>,
    pub max_session_duration: Option<i32>,
    pub permissions_boundary: Option<SerializableAttachedPermissionsBoundary>,
    pub tags: Option<Vec<SerializableTag>>,
}
impl From<types::Role> for SerializableRole {
    fn from(value: types::Role) -> Self {
        Self {
            path: value.path,
            role_name: value.role_name,
            role_id: value.role_id,
            arn: value.arn,
            create_date: value.create_date.secs(),
            assume_role_policy_document: value.assume_role_policy_document,
            description: value.description,
            max_session_duration: value.max_session_duration,
            permissions_boundary: value
                .permissions_boundary
                .map(SerializableAttachedPermissionsBoundary::from),
            tags: value
                .tags
                .map(|v| v.into_iter().map(SerializableTag::from).collect()),
        }
    }
}
impl From<Tag> for SerializableTag {
    fn from(value: Tag) -> Self {
        Self {
            key: value.key,
            value: value.value,
        }
    }
}

impl From<AttachedPermissionsBoundary> for SerializableAttachedPermissionsBoundary {
    fn from(value: AttachedPermissionsBoundary) -> Self {
        Self {
            permissions_boundary_type: value
                .permissions_boundary_type
                .map(SerializablePermissionsBoundaryAttachmentType::from),
            permissions_boundary_arn: value.permissions_boundary_arn,
        }
    }
}
impl From<PermissionsBoundaryAttachmentType> for SerializablePermissionsBoundaryAttachmentType {
    fn from(value: PermissionsBoundaryAttachmentType) -> Self {
        match value {
            PermissionsBoundaryAttachmentType::Policy => {
                SerializablePermissionsBoundaryAttachmentType::Policy
            }
            _ => SerializablePermissionsBoundaryAttachmentType::Unknown,
        }
    }
}

impl From<RoleLastUsed> for SerializableRoleLastUsed {
    fn from(value: RoleLastUsed) -> Self {
        SerializableRoleLastUsed {
            last_used_date: value.last_used_date.map(|d| d.secs()),
            region: value.region,
        }
    }
}

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct SerializableRoleLastUsed {
    pub last_used_date: Option<i64>,
    pub region: Option<String>,
}
#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct SerializableAttachedPermissionsBoundary {
    pub permissions_boundary_type: Option<SerializablePermissionsBoundaryAttachmentType>,
    pub permissions_boundary_arn: Option<String>,
}

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub enum SerializablePermissionsBoundaryAttachmentType {
    #[default]
    Policy,
    Unknown,
}

#[derive(Default, Serialize, Deserialize, Debug, Clone)]

pub struct SerializableCreateRoleInput {
    pub path: Option<String>,
    pub role_name: Option<String>,
    pub assume_role_policy_document: Option<String>,
    pub description: Option<String>,
    pub max_session_duration: Option<i32>,
    pub permissions_boundary: Option<String>,
    pub tags: Option<Vec<SerializableTag>>,
}

impl SerializableCreateRoleInput {
    pub fn to_aws_input(
        self,
        client: &aws_sdk_iam::Client,
    ) -> anyhow::Result<CreateRoleFluentBuilder> {
        Ok(client
            .create_role()
            .set_path(self.path)
            .set_role_name(self.role_name)
            .set_assume_role_policy_document(self.assume_role_policy_document)
            .set_description(self.description)
            .set_max_session_duration(self.max_session_duration)
            .set_permissions_boundary(self.permissions_boundary)
            .set_tags(if let Some(v) = self.tags {
                v.into_iter()
                    .try_fold(
                        vec![],
                        |mut acc: Vec<Tag>, item| -> anyhow::Result<Vec<Tag>> {
                            acc.push(Tag::try_from(item)?);
                            Ok(acc)
                        },
                    )
                    .map(Some)?
            } else {
                None
            }))
    }
}
impl TryFrom<SerializableTag> for Tag {
    type Error = anyhow::Error;
    fn try_from(value: SerializableTag) -> Result<Self, Self::Error> {
        TagBuilder::default()
            .key(value.key)
            .value(value.value)
            .build()
            .context("Tag build failure")
    }
}
