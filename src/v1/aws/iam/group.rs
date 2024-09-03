use crate::{
    prelude::{ArnProvider, AwsManager, AwsResource, AwsResourceCreator},
    v1::manager::{ManagerError, ResourceManager},
};
use aws_sdk_iam::{operation::create_group::builders::CreateGroupFluentBuilder, types::Group};
use aws_sdk_iam::{operation::get_group::GetGroupOutput, Client};
use serde::{Deserialize, Serialize};

pub type GroupInput = SerializableCreateGroupInput;
pub type GroupOutput = SerializableGetGroupOutput;
pub type GroupManager = AwsManager<GroupInput, GroupOutput, Client>;
pub type IAMGroup<'a> = AwsResource<'a, GroupInput, GroupOutput>;

impl ArnProvider for GroupOutput {
    fn arn(&self) -> Option<Vec<String>> {
        self.group.as_ref().map(|x| vec![x.arn.clone()])
    }
}

impl AwsResourceCreator for IAMGroup<'_> {
    type Input = GroupInput;
    type Output = GroupOutput;
    fn r#type() -> crate::prelude::AwsType {
        crate::prelude::AwsType::Group
    }
    fn manager(
        handle: &tokio::runtime::Handle,
        config: &aws_config::SdkConfig,
    ) -> std::sync::Arc<dyn ResourceManager<Self::Input, Self::Output>> {
        GroupManager::new(handle, Client::new(config), config).arc()
    }
    fn input_hook(_id: &str, _input: &mut Self::Input) {}
}
impl GroupManager {
    fn lookup(
        &self,
        group_name: &str,
    ) -> Result<Option<GroupOutput>, crate::v1::manager::ManagerError> {
        self.handle.block_on(async {
            self.client
                .get_group()
                .group_name(group_name)
                .send()
                .await
                .map(GroupOutput::from)
                .map(Some)
                .map_err(|e| ManagerError::LookupFail(format!("{:?}", e.into_source())))
                .or_else(|e| match e {
                    ManagerError::LookupFail(ref msg) if msg.contains("NoSuchEntity") => {
                        println!("Group Cannot be found");
                        Ok(None)
                    }
                    _ => Err(e),
                })
        })
    }
    fn create(
        &self,
        input: &mut GroupInput,
    ) -> Result<GroupOutput, crate::v1::manager::ManagerError> {
        self.handle.block_on(async {
            input
                .clone()
                .to_aws_input(&self.client)
                .send()
                .await
                .map_err(|e| ManagerError::CreateFail(format!("{:?}", e.into_source())))
        })?;
        if let Some(group) = self.lookup_by_input(input)? {
            Ok(group)
        } else {
            Err(ManagerError::CreateFail("Group not found".to_string()))
        }
    }
    fn delete(&self, group_name: &str) -> Result<bool, crate::v1::manager::ManagerError> {
        self.handle.block_on(async {
            self.client
                .delete_group()
                .group_name(group_name)
                .send()
                .await
                .map_err(|e| ManagerError::DeleteFail(format!("{:?}", e.into_source())))
                .map(|_| true)
        })
    }
}

impl ResourceManager<GroupInput, GroupOutput> for GroupManager {
    fn lookup(
        &self,
        latest: &GroupOutput,
    ) -> Result<Option<GroupOutput>, crate::v1::manager::ManagerError> {
        let group_name = latest
            .group
            .as_ref()
            .map(|x| x.group_name.as_str())
            .unwrap_or("");
        self.lookup(group_name)
    }

    fn lookup_by_input(
        &self,
        input: &GroupInput,
    ) -> Result<Option<GroupOutput>, crate::v1::manager::ManagerError> {
        let group_name = input.group_name.as_deref().unwrap_or("");
        self.lookup(group_name)
    }

    fn create(
        &self,
        input: &mut GroupInput,
    ) -> Result<GroupOutput, crate::v1::manager::ManagerError> {
        self.create(input)
    }

    fn delete(&self, latest: &GroupOutput) -> Result<bool, crate::v1::manager::ManagerError> {
        let group_name = latest
            .group
            .as_ref()
            .map(|x| x.group_name.as_str())
            .unwrap_or("");
        self.delete(group_name)
    }

    fn syncup(
        &self,
        _latest: &GroupOutput,
        _input: &mut GroupInput,
    ) -> Result<Option<GroupOutput>, crate::v1::manager::ManagerError> {
        Ok(None)
    }
}

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct SerializableCreateGroupInput {
    pub path: Option<String>,
    pub group_name: Option<String>,
}

impl SerializableCreateGroupInput {
    pub fn to_aws_input(self, client: &Client) -> CreateGroupFluentBuilder {
        client
            .create_group()
            .set_group_name(self.group_name)
            .set_path(self.path)
    }
}

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct SerializableGetGroupOutput {
    pub group: Option<SerializableGroup>,
    pub is_truncated: bool,
    pub marker: Option<String>,
}

impl From<GetGroupOutput> for SerializableGetGroupOutput {
    fn from(value: GetGroupOutput) -> Self {
        Self {
            group: value.group.map(|x| x.into()),
            is_truncated: value.is_truncated,
            marker: value.marker,
        }
    }
}

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct SerializableGroup {
    pub path: String,
    pub group_name: String,
    pub group_id: String,
    pub arn: String,
}

impl From<Group> for SerializableGroup {
    fn from(value: Group) -> Self {
        Self {
            path: value.path,
            group_name: value.group_name,
            group_id: value.group_id,
            arn: value.arn,
        }
    }
}
