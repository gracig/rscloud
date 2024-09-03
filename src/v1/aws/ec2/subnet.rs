use crate::{
    prelude::{AwsManager, AwsResource, AwsResourceCreator, AwsType, ResourceError},
    v1::manager::{ManagerError, ResourceManager},
};

use super::vpc::{SerializableTag, Vpc};
use aws_sdk_ec2::{
    operation::create_subnet::builders::CreateSubnetFluentBuilder,
    types::{
        self, HostnameType, PrivateDnsNameOptionsOnLaunch, ResourceType, SubnetCidrBlockState,
        SubnetCidrBlockStateCode, SubnetIpv6CidrBlockAssociation, SubnetState, TagSpecification,
    },
    Client,
};
use ipnet::Ipv4Net;
use serde::{Deserialize, Serialize};

pub type SubnetInput = SerializableCreateSubnetInput;
pub type SubnetOutput = SerializableSubnet;
pub type Subnet<'a> = AwsResource<'a, SubnetInput, SubnetOutput>;
type SubnetManager = AwsManager<SubnetInput, SubnetOutput, Client>;

impl AwsResourceCreator for Subnet<'_> {
    type Input = SubnetInput;
    type Output = SubnetOutput;
    fn r#type() -> crate::prelude::AwsType {
        AwsType::Subnet
    }
    fn manager(
        handle: &tokio::runtime::Handle,
        config: &aws_config::SdkConfig,
    ) -> std::sync::Arc<dyn ResourceManager<Self::Input, Self::Output>> {
        SubnetManager::new(handle, Client::new(config), config).arc()
    }

    fn input_hook(_id: &str, _input: &mut Self::Input) {}
}
impl<'a> Subnet<'a> {
    pub fn bind_vpc(
        self,
        vpc: &Vpc,
        position: usize,
        total_subnets: usize,
    ) -> Result<Subnet<'a>, ResourceError> {
        self.bind(vpc, move |this, other| {
            this.vpc_id.clone_from(&other.vpc_id);
            if let Some(cidr) = other.cidr_block.as_ref() {
                let net: Ipv4Net = cidr
                    .parse()
                    .unwrap_or_else(|_| panic!("Not a valid IPv4 Network: {}", cidr));
                let prefix_len = (total_subnets as f64).log2().ceil() as u8 + net.prefix_len();
                let mut subnets = net.subnets(prefix_len).expect("Subnets to be valid");
                let subnet = subnets.nth(position).expect("Subnet to exist");
                this.cidr_block = Some(subnet.to_string())
            }
        })?;
        Ok(self)
    }
}

impl ResourceManager<SubnetInput, SubnetOutput> for SubnetManager {
    fn lookup(
        &self,
        latest: &SubnetOutput,
    ) -> Result<Option<SubnetOutput>, crate::v1::manager::ManagerError> {
        match latest.subnet_id.as_ref() {
            None => Ok(None),
            Some(id) => self.lookup(id),
        }
    }
    fn create(
        &self,
        input: &mut SubnetInput,
    ) -> Result<SubnetOutput, crate::v1::manager::ManagerError> {
        self.create(input)
    }

    fn delete(&self, latest: &SubnetOutput) -> Result<bool, crate::v1::manager::ManagerError> {
        match latest.subnet_id.as_ref() {
            None => Ok(false),
            Some(id) => self.delete(id),
        }
    }

    fn syncup(
        &self,
        latest: &SubnetOutput,
        input: &mut SubnetInput,
    ) -> Result<Option<SubnetOutput>, crate::v1::manager::ManagerError> {
        Ok(match latest.subnet_id.as_ref() {
            Some(id) => match self.lookup(id)? {
                Some(r) => Some(r),
                None => Some(self.create(input)?),
            },
            None => Some(self.create(input)?),
        })
    }

    fn lookup_by_input(&self, _input: &SubnetInput) -> Result<Option<SubnetOutput>, ManagerError> {
        Ok(None)
    }
}

impl SubnetManager {
    pub fn lookup(&self, id: &str) -> Result<Option<SubnetOutput>, ManagerError> {
        self.handle.block_on(async {
            self.client
                .describe_subnets()
                .set_subnet_ids(Some(vec![id.to_string()]))
                .send()
                .await
                .map_err(|e| ManagerError::LookupFail(format!("{:?}", e.into_source())))
                .and_then(|response| match response.subnets {
                    None => Ok(None),
                    Some(r) => {
                        if r.is_empty() {
                            Ok(None)
                        } else if r.len() > 1 {
                            Err(ManagerError::LookupFail("Too many Subnets".to_string()))
                        } else {
                            Ok(Some(r[0].clone().into()))
                        }
                    }
                })
                .or_else(|e| match e {
                    ManagerError::LookupFail(ref msg) if msg.contains("NotFound") => Ok(None),
                    _ => Err(e),
                })
        })
    }
    fn create(&self, input: &mut SubnetInput) -> Result<SubnetOutput, ManagerError> {
        self.handle.block_on(async {
            input
                .clone()
                .to_aws_input(&self.client)
                .send()
                .await
                .map_err(|e| ManagerError::CreateFail(format!("{:?}", e.into_source())))
                .and_then(|response| match response.subnet {
                    None => Err(ManagerError::CreateFail("Subnet not created".to_string())),
                    Some(r) => Ok(r.into()),
                })
        })
    }
    fn delete(&self, id: &str) -> Result<bool, ManagerError> {
        self.handle.block_on(async {
            self.client
                .delete_subnet()
                .set_subnet_id(Some(id.to_string()))
                .send()
                .await
                .map_err(|e| {
                    ManagerError::DeleteFail(format!("Subnet not deleted: {:?}", e.into_source()))
                })
                .map(|_| true)
        })
    }
}

impl From<types::Subnet> for SerializableSubnet {
    fn from(item: types::Subnet) -> Self {
        SerializableSubnet {
            availability_zone: item.availability_zone,
            availability_zone_id: item.availability_zone_id,
            available_ip_address_count: item.available_ip_address_count,
            cidr_block: item.cidr_block,
            default_for_az: item.default_for_az,
            enable_lni_at_device_index: item.enable_lni_at_device_index,
            map_public_ip_on_launch: item.map_public_ip_on_launch,
            map_customer_owned_ip_on_launch: item.map_customer_owned_ip_on_launch,
            customer_owned_ipv4_pool: item.customer_owned_ipv4_pool,
            state: item.state.map(SerializedSubnetState::from), // Assuming you have an implementation for this conversion
            subnet_id: item.subnet_id,
            vpc_id: item.vpc_id,
            owner_id: item.owner_id,
            assign_ipv6_address_on_creation: item.assign_ipv6_address_on_creation,
            ipv6_cidr_block_association_set: item.ipv6_cidr_block_association_set.map(|vec| {
                vec.into_iter()
                    .map(SerializableSubnetIpv6CidrBlockAssociation::from)
                    .collect()
            }),
            tags: item
                .tags
                .map(|vec| vec.into_iter().map(SerializableTag::from).collect()), // Assuming you have an implementation for this conversion
            subnet_arn: item.subnet_arn,
            outpost_arn: item.outpost_arn,
            enable_dns64: item.enable_dns64,
            ipv6_native: item.ipv6_native,
            private_dns_name_options_on_launch: item
                .private_dns_name_options_on_launch
                .map(SerializablePrivateDnsNameOptionsOnLaunch::from), // Assuming you have an implementation for this conversion
        }
    }
}

impl SerializableCreateSubnetInput {
    pub fn to_aws_input(self, client: &Client) -> CreateSubnetFluentBuilder {
        let mut builder = client.create_subnet();

        if let Some(v) = self.tag_specifications {
            let mut spec = TagSpecification::builder().resource_type(ResourceType::Subnet);
            for v in v {
                spec = spec.tags(v.into());
            }
            builder = builder.tag_specifications(spec.build());
        }
        if let Some(zone) = self.availability_zone {
            builder = builder.availability_zone(zone);
        }
        if let Some(zone_id) = self.availability_zone_id {
            builder = builder.availability_zone_id(zone_id);
        }
        if let Some(block) = self.cidr_block {
            builder = builder.cidr_block(block);
        }
        if let Some(ipv6_block) = self.ipv6_cidr_block {
            builder = builder.ipv6_cidr_block(ipv6_block);
        }
        if let Some(arn) = self.outpost_arn {
            builder = builder.outpost_arn(arn);
        }
        if let Some(vpc) = self.vpc_id {
            builder = builder.vpc_id(vpc);
        }
        if let Some(dry) = self.dry_run {
            builder = builder.dry_run(dry);
        }
        if let Some(native) = self.ipv6_native {
            builder = builder.ipv6_native(native);
        }
        if let Some(ipam_pool_id) = self.ipv4_ipam_pool_id {
            builder = builder.ipv4_ipam_pool_id(ipam_pool_id);
        }
        if let Some(netmask_length) = self.ipv4_netmask_length {
            builder = builder.ipv4_netmask_length(netmask_length);
        }
        if let Some(ipv6_pool_id) = self.ipv6_ipam_pool_id {
            builder = builder.ipv6_ipam_pool_id(ipv6_pool_id);
        }
        if let Some(ipv6_netmask_length) = self.ipv6_netmask_length {
            builder = builder.ipv6_netmask_length(ipv6_netmask_length);
        }

        builder
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableSubnet {
    pub availability_zone: Option<String>,
    pub availability_zone_id: Option<String>,
    pub available_ip_address_count: Option<i32>,
    pub cidr_block: Option<String>,
    pub default_for_az: Option<bool>,
    pub enable_lni_at_device_index: Option<i32>,
    pub map_public_ip_on_launch: Option<bool>,
    pub map_customer_owned_ip_on_launch: Option<bool>,
    pub customer_owned_ipv4_pool: Option<String>,
    pub state: Option<SerializedSubnetState>,
    pub subnet_id: Option<String>,
    pub vpc_id: Option<String>,
    pub owner_id: Option<String>,
    pub assign_ipv6_address_on_creation: Option<bool>,
    pub ipv6_cidr_block_association_set: Option<Vec<SerializableSubnetIpv6CidrBlockAssociation>>,
    pub tags: Option<Vec<SerializableTag>>,
    pub subnet_arn: Option<String>,
    pub outpost_arn: Option<String>,
    pub enable_dns64: Option<bool>,
    pub ipv6_native: Option<bool>,
    pub private_dns_name_options_on_launch: Option<SerializablePrivateDnsNameOptionsOnLaunch>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializablePrivateDnsNameOptionsOnLaunch {
    pub hostname_type: Option<SerializableHostnameType>,
    pub enable_resource_name_dns_a_record: Option<bool>,
    pub enable_resource_name_dns_aaaa_record: Option<bool>,
}
impl From<PrivateDnsNameOptionsOnLaunch> for SerializablePrivateDnsNameOptionsOnLaunch {
    fn from(state: PrivateDnsNameOptionsOnLaunch) -> Self {
        SerializablePrivateDnsNameOptionsOnLaunch {
            hostname_type: state.hostname_type.map(|s| s.into()),
            enable_resource_name_dns_a_record: state.enable_resource_name_dns_a_record,
            enable_resource_name_dns_aaaa_record: state.enable_resource_name_dns_aaaa_record,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum SerializableHostnameType {
    IpName,
    ResourceName,
    Unknown,
}
impl From<HostnameType> for SerializableHostnameType {
    fn from(state: HostnameType) -> Self {
        match state {
            HostnameType::IpName => SerializableHostnameType::IpName,
            HostnameType::ResourceName => SerializableHostnameType::ResourceName,
            _ => SerializableHostnameType::Unknown,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableSubnetIpv6CidrBlockAssociation {
    pub association_id: Option<String>,
    pub ipv6_cidr_block: Option<String>,
    pub ipv6_cidr_block_state: Option<SerializableSubnetCidrBlockState>,
}
impl From<SubnetIpv6CidrBlockAssociation> for SerializableSubnetIpv6CidrBlockAssociation {
    fn from(state: SubnetIpv6CidrBlockAssociation) -> Self {
        SerializableSubnetIpv6CidrBlockAssociation {
            association_id: state.association_id,
            ipv6_cidr_block: state.ipv6_cidr_block,
            ipv6_cidr_block_state: state.ipv6_cidr_block_state.map(|s| s.into()),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableSubnetCidrBlockState {
    pub state: Option<SerializableSubnetCidrBlockStateCode>,
    pub status_message: Option<String>,
}
impl From<SubnetCidrBlockState> for SerializableSubnetCidrBlockState {
    fn from(state: SubnetCidrBlockState) -> Self {
        SerializableSubnetCidrBlockState {
            state: state.state.map(|s| s.into()),
            status_message: state.status_message,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum SerializableSubnetCidrBlockStateCode {
    Associated,
    Associating,
    Disassociated,
    Disassociating,
    Failed,
    Failing,
    Unknown,
}
impl From<SubnetCidrBlockStateCode> for SerializableSubnetCidrBlockStateCode {
    fn from(state: SubnetCidrBlockStateCode) -> Self {
        match state {
            SubnetCidrBlockStateCode::Associated => {
                SerializableSubnetCidrBlockStateCode::Associated
            }
            SubnetCidrBlockStateCode::Associating => {
                SerializableSubnetCidrBlockStateCode::Associating
            }
            SubnetCidrBlockStateCode::Disassociated => {
                SerializableSubnetCidrBlockStateCode::Disassociated
            }
            SubnetCidrBlockStateCode::Disassociating => {
                SerializableSubnetCidrBlockStateCode::Disassociating
            }
            SubnetCidrBlockStateCode::Failed => SerializableSubnetCidrBlockStateCode::Failed,
            SubnetCidrBlockStateCode::Failing => SerializableSubnetCidrBlockStateCode::Failing,
            _ => SerializableSubnetCidrBlockStateCode::Unknown,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum SerializedSubnetState {
    Available,
    Pending,
    Unavailable,
    Unknown,
}

impl From<SubnetState> for SerializedSubnetState {
    fn from(state: SubnetState) -> Self {
        match state {
            SubnetState::Available => SerializedSubnetState::Available,
            SubnetState::Pending => SerializedSubnetState::Pending,
            SubnetState::Unavailable => SerializedSubnetState::Unavailable,
            _ => SerializedSubnetState::Unknown,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct SerializableCreateSubnetInput {
    /// <p>The tags to assign to the subnet.</p>
    pub tag_specifications: Option<Vec<SerializableTag>>,
    /// <p>The Availability Zone or Local Zone for the subnet.</p>
    /// <p>Default: Amazon Web Services selects one for you. If you create more than one subnet in your VPC, we do not necessarily select a different zone for each subnet.</p>
    /// <p>To create a subnet in a Local Zone, set this value to the Local Zone ID, for example <code>us-west-2-lax-1a</code>. For information about the Regions that support Local Zones, see <a href="http://aws.amazon.com/about-aws/global-infrastructure/localzones/locations/">Local Zones locations</a>.</p>
    /// <p>To create a subnet in an Outpost, set this value to the Availability Zone for the Outpost and specify the Outpost ARN.</p>
    pub availability_zone: ::std::option::Option<::std::string::String>,
    /// <p>The AZ ID or the Local Zone ID of the subnet.</p>
    pub availability_zone_id: ::std::option::Option<::std::string::String>,
    /// <p>The IPv4 network range for the subnet, in CIDR notation. For example, <code>10.0.0.0/24</code>. We modify the specified CIDR block to its canonical form; for example, if you specify <code>100.68.0.18/18</code>, we modify it to <code>100.68.0.0/18</code>.</p>
    /// <p>This parameter is not supported for an IPv6 only subnet.</p>
    pub cidr_block: ::std::option::Option<::std::string::String>,
    /// <p>The IPv6 network range for the subnet, in CIDR notation. This parameter is required for an IPv6 only subnet.</p>
    pub ipv6_cidr_block: ::std::option::Option<::std::string::String>,
    /// <p>The Amazon Resource Name (ARN) of the Outpost. If you specify an Outpost ARN, you must also specify the Availability Zone of the Outpost subnet.</p>
    pub outpost_arn: ::std::option::Option<::std::string::String>,
    /// <p>The ID of the VPC.</p>
    pub vpc_id: ::std::option::Option<::std::string::String>,
    /// <p>Checks whether you have the required permissions for the action, without actually making the request, and provides an error response. If you have the required permissions, the error response is <code>DryRunOperation</code>. Otherwise, it is <code>UnauthorizedOperation</code>.</p>
    pub dry_run: ::std::option::Option<bool>,
    /// <p>Indicates whether to create an IPv6 only subnet.</p>
    pub ipv6_native: ::std::option::Option<bool>,
    /// <p>An IPv4 IPAM pool ID for the subnet.</p>
    pub ipv4_ipam_pool_id: ::std::option::Option<::std::string::String>,
    /// <p>An IPv4 netmask length for the subnet.</p>
    pub ipv4_netmask_length: ::std::option::Option<i32>,
    /// <p>An IPv6 IPAM pool ID for the subnet.</p>
    pub ipv6_ipam_pool_id: ::std::option::Option<::std::string::String>,
    /// <p>An IPv6 netmask length for the subnet.</p>
    pub ipv6_netmask_length: ::std::option::Option<i32>,
}
