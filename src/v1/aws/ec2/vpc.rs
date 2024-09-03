use crate::prelude::{AwsManager, AwsResource, AwsResourceCreator, AwsType, ResourceState};
use crate::v1::manager::{ManagerError, ResourceManager};
use crate::v1::plan::ResourceItem;
use aws_config::SdkConfig;
use aws_sdk_ec2::types::{self, AttributeBooleanValue, VpcAttributeName};
use aws_sdk_ec2::{
    operation::create_vpc::builders::CreateVpcFluentBuilder,
    types::{
        ResourceType, Tag, TagSpecification, Tenancy, VpcCidrBlockAssociation, VpcCidrBlockState,
        VpcCidrBlockStateCode, VpcIpv6CidrBlockAssociation, VpcState,
    },
    Client,
};
use serde::{Deserialize, Serialize};
use std::error::Error;
use tokio::runtime::Handle;

use super::subnet::{Subnet, SubnetInput};

pub type VpcOutput = SerializableVpc;
pub type VpcInput = SerializableCreateVpcInput;
pub type VpcManager = AwsManager<VpcInput, VpcOutput, Client>;
pub type Vpc<'a> = AwsResource<'a, VpcInput, VpcOutput>;
impl AwsResourceCreator for Vpc<'_> {
    type Input = VpcInput;
    type Output = VpcOutput;
    fn r#type() -> crate::prelude::AwsType {
        AwsType::Vpc
    }
    fn manager(
        handle: &Handle,
        config: &SdkConfig,
    ) -> std::sync::Arc<dyn ResourceManager<Self::Input, Self::Output>> {
        VpcManager::new(handle, Client::new(config), config).arc()
    }

    fn input_hook(_id: &str, _input: &mut Self::Input) {}
}

impl<'a> Vpc<'a> {
    pub fn subnets(&self, subnet_inputs: Vec<SubnetInput>) -> anyhow::Result<Vec<Subnet>> {
        let vpc_name = self.inner.name();
        let size = subnet_inputs.len();
        subnet_inputs
            .into_iter()
            .enumerate()
            .try_fold(vec![], |mut acc, (index, item)| {
                let subnet_id = format!("{}-subnet{}", vpc_name, index);
                acc.push(
                    self.aws
                        .resource::<Subnet>(&subnet_id, ResourceState::Present, item)?
                        .bind_vpc(self, index, size)?,
                );
                Ok(acc)
            })
    }
}

impl ResourceManager<VpcInput, VpcOutput> for VpcManager {
    fn create(&self, input: &mut VpcInput) -> Result<VpcOutput, ManagerError> {
        let create_vpc_input = input;
        let mut vpc = self.create_vpc(create_vpc_input)?;
        let vpc_id = vpc
            .vpc_id
            .as_ref()
            .ok_or(ManagerError::CreateFail("Could not get VPC ID".to_string()))?;
        let attributes = SerializableCreateVpcAttributes {
            enable_dns_support: match create_vpc_input.attributes.enable_dns_support {
                Some(value) => {
                    self.set_attribute(vpc_id, VpcAttributeName::EnableDnsSupport, value)?
                }
                None => None,
            },
            enable_dns_hostnames: match create_vpc_input.attributes.enable_dns_hostnames {
                Some(value) => {
                    self.set_attribute(vpc_id, VpcAttributeName::EnableDnsHostnames, value)?
                }
                None => None,
            },
            enable_network_address_usage_metrics: match create_vpc_input
                .attributes
                .enable_network_address_usage_metrics
            {
                Some(value) => self.set_attribute(
                    vpc_id,
                    VpcAttributeName::EnableNetworkAddressUsageMetrics,
                    value,
                )?,
                None => None,
            },
        };
        vpc.attributes = attributes;
        Ok(vpc)
    }
    fn lookup(&self, latest: &VpcOutput) -> Result<Option<VpcOutput>, ManagerError> {
        let vpc = self.lookup_vpc(latest)?;
        match vpc {
            None => Ok(None),
            Some(mut vpc) => {
                let vpc_id = vpc.vpc_id.as_ref().ok_or(ManagerError::LookupFail(
                    "Could not retrieve vpc id".to_string(),
                ))?;
                let attributes = SerializableCreateVpcAttributes {
                    enable_dns_support: self
                        .lookup_attribute(vpc_id, VpcAttributeName::EnableDnsSupport)?,
                    enable_dns_hostnames: self
                        .lookup_attribute(vpc_id, VpcAttributeName::EnableDnsHostnames)?,
                    enable_network_address_usage_metrics: self.lookup_attribute(
                        vpc_id,
                        VpcAttributeName::EnableNetworkAddressUsageMetrics,
                    )?,
                };
                vpc.attributes = attributes;
                Ok(Some(vpc))
            }
        }
    }
    fn delete(&self, latest: &VpcOutput) -> Result<bool, ManagerError> {
        self.lookup_vpc(latest).and_then({
            |vpc| match vpc {
                None => Ok(false),
                Some(vpc) => vpc
                    .vpc_id
                    .map(|vpc_id| self.delete_vpc(&vpc_id))
                    .unwrap_or(Ok(false)),
            }
        })
    }
    fn syncup(
        &self,
        latest: &VpcOutput,
        input: &mut VpcInput,
    ) -> Result<Option<VpcOutput>, ManagerError> {
        let create_vpc_input = input;
        self.handle.block_on(async {
            if create_vpc_input.cidr_block.as_deref() != latest.cidr_block.as_deref() {
                return Err(ManagerError::CannotSyncWithoutRecreate(format!(
                    "cidr_block from value: [{:?}] to value: [{:?}]",
                    latest.cidr_block,
                    create_vpc_input.cidr_block.as_deref()
                )));
            }
            Ok(None)
        })
    }

    fn lookup_by_input(&self, _input: &VpcInput) -> Result<Option<VpcOutput>, ManagerError> {
        Ok(None)
    }
}

impl VpcManager {
    fn create_vpc(
        &self,
        input: &mut SerializableCreateVpcInput,
    ) -> Result<SerializableVpc, ManagerError> {
        self.handle.block_on(async {
            input
                .clone()
                .create_vpc_input(&self.client)
                .send()
                .await
                .map_err(|e| ManagerError::CreateFail(format!("{:?}", e.source())))
                .and_then(|response| match response.vpc {
                    None => Err(ManagerError::CreateFail("VPC not created".to_string())),
                    Some(vpc) => Ok(vpc.into()),
                })
        })
    }
    fn delete_vpc(&self, vpc_id: &str) -> Result<bool, ManagerError> {
        self.handle.block_on(async {
            self.client
                .delete_vpc()
                .vpc_id(vpc_id)
                .send()
                .await
                .map_err(|e| ManagerError::DeleteFail(format!("{:?}", e.into_source())))
                .map(|_| true)
        })
    }
    fn set_attribute(
        &self,
        vpc_id: &str,
        attribute: VpcAttributeName,
        value: bool,
    ) -> Result<Option<bool>, ManagerError> {
        let attribute_value = AttributeBooleanValue::builder()
            .set_value(Some(value))
            .build();
        {
            let attribute = &attribute;
            let mut modify = self.client.modify_vpc_attribute().vpc_id(vpc_id);
            if let VpcAttributeName::EnableDnsHostnames = attribute {
                modify = modify.set_enable_dns_hostnames(Some(attribute_value));
            } else if let VpcAttributeName::EnableDnsSupport = attribute {
                modify = modify.set_enable_dns_hostnames(Some(attribute_value))
            } else if let VpcAttributeName::EnableNetworkAddressUsageMetrics = attribute {
                modify = modify.set_enable_network_address_usage_metrics(Some(attribute_value))
            }
            self.handle.block_on(async {
                modify
                    .send()
                    .await
                    .map_err(|e| ManagerError::CreateFail(format!("{:?}", e.source())))
            })?;
        }
        self.lookup_attribute(vpc_id, attribute)
    }
    fn lookup_attribute(
        &self,
        vpc_id: &str,
        attribute: VpcAttributeName,
    ) -> Result<Option<bool>, ManagerError> {
        self.handle.block_on(async {
            self.client
                .describe_vpc_attribute()
                .vpc_id(vpc_id)
                .attribute(attribute.clone())
                .send()
                .await
                .map_err(|e| ManagerError::LookupFail(format!("{:?}", e.source())))
                .map(|response| match attribute {
                    VpcAttributeName::EnableDnsHostnames => {
                        response.enable_dns_hostnames.and_then(|v| v.value)
                    }
                    VpcAttributeName::EnableDnsSupport => {
                        response.enable_dns_support.and_then(|v| v.value())
                    }
                    VpcAttributeName::EnableNetworkAddressUsageMetrics => response
                        .enable_network_address_usage_metrics
                        .and_then(|v| v.value()),
                    _ => None,
                })
        })
    }
    fn lookup_vpc(&self, vpc: &VpcOutput) -> Result<Option<SerializableVpc>, ManagerError> {
        if vpc.vpc_id.is_none() {
            return Err(ManagerError::LookupFail("Vpc id is none".to_string()));
        }
        self.handle.block_on(async {
            self.client
                .describe_vpcs()
                .vpc_ids(vpc.vpc_id.clone().unwrap())
                .send()
                .await
                .map_err(|e| ManagerError::LookupFail(format!("{:?}", e.source())))
                .and_then(|response| match response.vpcs {
                    None => Ok(None),
                    Some(vpcs) => {
                        if vpcs.is_empty() {
                            Ok(None)
                        } else if vpcs.len() > 1 {
                            Err(ManagerError::LookupFail("Too many VPCs".to_string()))
                        } else {
                            Ok(Some(vpcs[0].clone().into()))
                        }
                    }
                })
                .or_else(|e| match e {
                    ManagerError::LookupFail(ref msg) if msg.contains("NotFound") => Ok(None),
                    _ => Err(e),
                })
        })
    }
}

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct SerializableCreateVpcAttributes {
    pub enable_dns_support: Option<bool>,
    pub enable_dns_hostnames: Option<bool>,
    pub enable_network_address_usage_metrics: Option<bool>,
}

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct SerializableCreateVpcInput {
    pub cidr_block: Option<String>,
    pub amazon_provided_ipv6_cidr_block: Option<bool>,
    pub ipv6_pool: Option<String>,
    pub ipv6_cidr_block: Option<String>,
    pub ipv4_ipam_pool_id: Option<String>,
    pub ipv4_netmask_length: Option<i32>,
    pub ipv6_ipam_pool_id: Option<String>,
    pub ipv6_netmask_length: Option<i32>,
    pub dry_run: Option<bool>,
    pub instance_tenancy: Option<SerializableTenancy>,
    pub ipv6_cidr_block_network_border_group: Option<String>,
    pub tag_specifications: Option<Vec<SerializableTag>>,
    pub attributes: SerializableCreateVpcAttributes,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableVpc {
    pub cidr_block: Option<String>,
    pub dhcp_options_id: Option<String>,
    pub state: Option<SerializableVpcState>,
    pub vpc_id: Option<String>,
    pub owner_id: Option<String>,
    pub instance_tenancy: Option<SerializableTenancy>,
    pub ipv6_cidr_block_association_set: Option<Vec<SerializableVpcIpv6CidrBlockAssociation>>,
    pub cidr_block_association_set: Option<Vec<SerializableVpcCidrBlockAssociation>>,
    pub is_default: Option<bool>,
    pub tags: Option<Vec<SerializableTag>>,
    pub attributes: SerializableCreateVpcAttributes,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableVpcCidrBlockAssociation {
    pub association_id: Option<String>,
    pub cidr_block: Option<String>,
    pub cidr_block_state: Option<SerializableVpcCidrBlockState>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableVpcIpv6CidrBlockAssociation {
    pub association_id: Option<String>,
    pub ipv6_cidr_block: Option<String>,
    pub ipv6_cidr_block_state: Option<SerializableVpcCidrBlockState>,
    pub network_border_group: Option<String>,
    pub ipv6_pool: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableVpcCidrBlockState {
    pub state: Option<SerializableVpcCidrBlockStateCode>,
    pub status_message: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum SerializableVpcCidrBlockStateCode {
    Associated,
    Associating,
    Disassociated,
    Disassociating,
    Failed,
    Failing,
    Unknown,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum SerializableVpcState {
    Available,
    Pending,
    Unknown,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableTag {
    pub key: String,
    pub value: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum SerializableTenancy {
    Dedicated,
    Default,
    Host,
}

impl From<SerializableTenancy> for Tenancy {
    fn from(serializable: SerializableTenancy) -> Tenancy {
        match serializable {
            SerializableTenancy::Default => Tenancy::Default,
            SerializableTenancy::Dedicated => Tenancy::Dedicated,
            SerializableTenancy::Host => Tenancy::Host,
        }
    }
}

impl From<VpcState> for SerializableVpcState {
    fn from(state: VpcState) -> Self {
        match state {
            VpcState::Available => SerializableVpcState::Available,
            VpcState::Pending => SerializableVpcState::Pending,
            _ => SerializableVpcState::Unknown, // Handle other states as needed
        }
    }
}

impl From<VpcIpv6CidrBlockAssociation> for SerializableVpcIpv6CidrBlockAssociation {
    fn from(assoc: VpcIpv6CidrBlockAssociation) -> Self {
        SerializableVpcIpv6CidrBlockAssociation {
            association_id: assoc.association_id,
            ipv6_cidr_block: assoc.ipv6_cidr_block,
            ipv6_cidr_block_state: assoc.ipv6_cidr_block_state.map(Into::into),
            network_border_group: assoc.network_border_group,
            ipv6_pool: assoc.ipv6_pool,
        }
    }
}
impl From<VpcCidrBlockState> for SerializableVpcCidrBlockState {
    fn from(state: VpcCidrBlockState) -> Self {
        SerializableVpcCidrBlockState {
            state: state.state.map(|s| match s {
                VpcCidrBlockStateCode::Associated => SerializableVpcCidrBlockStateCode::Associated,
                VpcCidrBlockStateCode::Associating => {
                    SerializableVpcCidrBlockStateCode::Associating
                }
                VpcCidrBlockStateCode::Disassociated => {
                    SerializableVpcCidrBlockStateCode::Disassociated
                }
                VpcCidrBlockStateCode::Disassociating => {
                    SerializableVpcCidrBlockStateCode::Disassociating
                }
                VpcCidrBlockStateCode::Failed => SerializableVpcCidrBlockStateCode::Failed,
                VpcCidrBlockStateCode::Failing => SerializableVpcCidrBlockStateCode::Failing,
                _ => SerializableVpcCidrBlockStateCode::Unknown, // Add other cases as needed based on the SDK
                                                                 // Add other cases as needed based on the SDK
            }),
            status_message: state.status_message,
        }
    }
}
impl From<VpcCidrBlockAssociation> for SerializableVpcCidrBlockAssociation {
    fn from(assoc: VpcCidrBlockAssociation) -> Self {
        SerializableVpcCidrBlockAssociation {
            association_id: assoc.association_id,
            cidr_block: assoc.cidr_block,
            cidr_block_state: assoc.cidr_block_state.map(Into::into),
        }
    }
}

impl From<Tenancy> for SerializableTenancy {
    fn from(tenancy: Tenancy) -> SerializableTenancy {
        match tenancy {
            Tenancy::Default => SerializableTenancy::Default,
            Tenancy::Dedicated => SerializableTenancy::Dedicated,
            Tenancy::Host => SerializableTenancy::Host,
            _ => SerializableTenancy::Default,
        }
    }
}

impl From<types::Vpc> for SerializableVpc {
    fn from(vpc: types::Vpc) -> Self {
        SerializableVpc {
            cidr_block: vpc.cidr_block,
            dhcp_options_id: vpc.dhcp_options_id,
            state: vpc.state.map(Into::into),
            vpc_id: vpc.vpc_id,
            owner_id: vpc.owner_id,
            instance_tenancy: vpc.instance_tenancy.map(|it| it.into()),
            ipv6_cidr_block_association_set: vpc
                .ipv6_cidr_block_association_set
                .map(|vec| vec.into_iter().map(Into::into).collect()),
            cidr_block_association_set: vpc
                .cidr_block_association_set
                .map(|vec| vec.into_iter().map(Into::into).collect()),
            is_default: vpc.is_default,
            tags: vpc
                .tags
                .map(|vec| vec.into_iter().map(Into::into).collect()),
            attributes: Default::default(),
        }
    }
}

impl SerializableCreateVpcInput {
    pub fn create_vpc_input(self, client: &Client) -> CreateVpcFluentBuilder {
        let mut builder = client.create_vpc();
        if let Some(v) = self.cidr_block {
            builder = builder.cidr_block(v);
        }
        if let Some(v) = self.amazon_provided_ipv6_cidr_block {
            builder = builder.amazon_provided_ipv6_cidr_block(v);
        }
        if let Some(v) = self.ipv6_pool {
            builder = builder.ipv6_pool(v);
        }
        if let Some(v) = self.ipv6_cidr_block {
            builder = builder.ipv6_cidr_block(v);
        }
        if let Some(v) = self.ipv4_ipam_pool_id {
            builder = builder.ipv4_ipam_pool_id(v);
        }
        if let Some(v) = self.ipv4_netmask_length {
            builder = builder.ipv4_netmask_length(v);
        }
        if let Some(v) = self.ipv6_ipam_pool_id {
            builder = builder.ipv6_ipam_pool_id(v);
        }
        if let Some(v) = self.ipv6_netmask_length {
            builder = builder.ipv6_netmask_length(v);
        }
        if let Some(v) = self.dry_run {
            builder = builder.dry_run(v);
        }
        if let Some(v) = self.instance_tenancy {
            builder = builder.instance_tenancy(v.into());
        }
        if let Some(v) = self.ipv6_cidr_block_network_border_group {
            builder = builder.ipv6_cidr_block_network_border_group(v);
        }
        if let Some(v) = self.tag_specifications {
            let mut spec = TagSpecification::builder().resource_type(ResourceType::Vpc);
            for v in v {
                spec = spec.tags(v.into());
            }
            builder = builder.tag_specifications(spec.build());
        }
        builder
    }
}

impl From<SerializableVpcState> for VpcState {
    fn from(state: SerializableVpcState) -> Self {
        match state {
            SerializableVpcState::Available => VpcState::Available,
            SerializableVpcState::Pending => VpcState::Pending,
            SerializableVpcState::Unknown => VpcState::Pending,
        }
    }
}

impl From<SerializableTag> for Tag {
    fn from(serializable: SerializableTag) -> Tag {
        Tag::builder()
            .set_key(Some(serializable.key))
            .set_value(Some(serializable.value))
            .build()
    }
}

impl From<Tag> for SerializableTag {
    fn from(tag: Tag) -> Self {
        SerializableTag {
            key: tag.key.unwrap_or_default(),
            value: tag.value.unwrap_or_default(),
        }
    }
}
