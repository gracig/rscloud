use std::{
    collections::HashMap,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use aws_sdk_s3::{
    operation::{get_object::GetObjectOutput, put_object::builders::PutObjectFluentBuilder},
    primitives::{ByteStream, DateTime},
    types::*,
    Client,
};

use bucket::Bucket;
use serde::{Deserialize, Serialize};

use crate::{
    prelude::*,
    v1::manager::{ManagerError, ResourceManager},
};

use super::bucket::{
    SerializableChecksumAlgorithm, SerializableServerSideEncryption, SerializableStorageClass,
};

pub type S3ObjectInput = SerializablePutObjectInput;
pub type S3ObjectOutput = SerializableGetObjectOutput;
pub type S3ObjectManager = AwsManager<S3ObjectInput, S3ObjectOutput, Client>;
pub type S3Object<'a> = AwsResource<'a, S3ObjectInput, S3ObjectOutput>;

impl<'a> S3Object<'a> {
    pub fn bind_bucket(&self, bucket: &Bucket) -> Result<(), ResourceError> {
        self.bind(bucket, |me, dep| {
            me.bucket = Some(dep.bucket_name.clone());
        })
    }
}
impl ResourceManager<S3ObjectInput, S3ObjectOutput> for S3ObjectManager {
    fn lookup(
        &self,
        latest: &S3ObjectOutput,
    ) -> Result<Option<S3ObjectOutput>, crate::v1::manager::ManagerError> {
        let key = latest.key.as_ref().ok_or_else(|| {
            ManagerError::LookupFail("Key is required to lookup an object".to_string())
        })?;
        let bucket = latest.bucket.as_ref().ok_or_else(|| {
            ManagerError::LookupFail("Bucket is required to lookup an object".to_string())
        })?;
        self.lookup(key, bucket)
    }

    fn lookup_by_input(
        &self,
        input: &S3ObjectInput,
    ) -> Result<Option<S3ObjectOutput>, crate::v1::manager::ManagerError> {
        let key = input.key.as_ref().ok_or_else(|| {
            ManagerError::LookupFail("Key is required to lookup an object".to_string())
        })?;
        let bucket = input.bucket.as_ref().ok_or_else(|| {
            ManagerError::LookupFail("Bucket is required to lookup an object".to_string())
        })?;
        self.lookup(key, bucket)
    }

    fn create(
        &self,
        input: &mut S3ObjectInput,
    ) -> Result<S3ObjectOutput, crate::v1::manager::ManagerError> {
        self.create(input)
    }

    fn delete(&self, latest: &S3ObjectOutput) -> Result<bool, crate::v1::manager::ManagerError> {
        let key = latest.key.as_ref().ok_or_else(|| {
            ManagerError::DeleteFail("Key is required to delete an object".to_string())
        })?;
        let bucket = latest.bucket.as_ref().ok_or_else(|| {
            ManagerError::DeleteFail("Bucket is required to delete an object".to_string())
        })?;
        self.delete(key, bucket)
    }

    fn syncup(
        &self,
        _latest: &S3ObjectOutput,
        input: &mut S3ObjectInput,
    ) -> Result<Option<S3ObjectOutput>, crate::v1::manager::ManagerError> {
        self.create(input).map(Some)
    }
}

impl S3ObjectManager {
    fn lookup(&self, key: &str, bucket: &str) -> Result<Option<S3ObjectOutput>, ManagerError> {
        self.handle.block_on(async {
            self.client
                .get_object()
                .bucket(bucket)
                .key(key)
                .send()
                .await
                .map_err(|e| ManagerError::LookupFail(format!("{:?}", e.into_source())))
                .map(S3ObjectOutput::from)
                .map(|mut o| {
                    o.key = Some(key.to_string());
                    o.bucket = Some(bucket.to_string());
                    Some(o)
                })
                .or_else(|e| match e {
                    ManagerError::LookupFail(ref msg) if msg.contains("NoSuch") => {
                        println!("Object Cannot be found");
                        Ok(None)
                    }
                    _ => Err(e),
                })
        })
    }
    fn create(&self, input: &S3ObjectInput) -> Result<S3ObjectOutput, ManagerError> {
        let key = input.key.as_ref().ok_or_else(|| {
            ManagerError::CreateFail("Key is required to create an object".into())
        })?;
        let bucket = input.bucket.as_ref().ok_or_else(|| {
            ManagerError::CreateFail("Bucket is required to create an object".into())
        })?;
        self.handle.block_on(async {
            input
                .clone()
                .to_aws_input(&self.client)
                .map_err(|e| ManagerError::CreateFail(format!("{:?}", e)))?
                .send()
                .await
                .map_err(|e| ManagerError::CreateFail(format!("{:?}", e.into_source())))
        })?;
        self.lookup(key, bucket)?
            .ok_or_else(|| ManagerError::CreateFail("Object not found".into()))
    }
    fn delete(&self, key: &str, bucket: &str) -> Result<bool, ManagerError> {
        self.handle.block_on(async {
            self.client
                .delete_object()
                .key(key)
                .bucket(bucket)
                .send()
                .await
                .map_err(|e| ManagerError::DeleteFail(format!("{:?}", e.into_source())))
                .map(|_| true)
        })
    }
}

impl AwsResourceCreator for S3Object<'_> {
    type Input = S3ObjectInput;
    type Output = S3ObjectOutput;
    fn r#type() -> crate::prelude::AwsType {
        crate::prelude::AwsType::S3Object
    }
    fn manager(
        handle: &tokio::runtime::Handle,
        config: &aws_config::SdkConfig,
    ) -> std::sync::Arc<dyn crate::v1::manager::ResourceManager<Self::Input, Self::Output>> {
        S3ObjectManager::new(handle, Client::new(config), config).arc()
    }
    fn input_hook(_id: &str, _input: &mut Self::Input) {}
}

impl SerializablePutObjectInput {
    pub fn to_aws_input(self, client: &Client) -> anyhow::Result<PutObjectFluentBuilder> {
        Ok(client
            .put_object()
            .set_acl(self.acl.map(ObjectCannedAcl::from))
            .body(ByteStream::from(self.body))
            .set_bucket(self.bucket)
            .set_bucket_key_enabled(self.bucket_key_enabled)
            .set_cache_control(self.cache_control)
            .set_content_disposition(self.content_disposition)
            .set_content_encoding(self.content_encoding)
            .set_content_language(self.content_language)
            .set_content_length(self.content_length)
            .set_content_md5(self.content_md5)
            .set_content_type(self.content_type)
            .set_checksum_algorithm(self.checksum_algorithm.map(Into::into))
            .set_checksum_crc32(self.checksum_crc32)
            .set_checksum_crc32_c(self.checksum_crc32_c)
            .set_checksum_sha1(self.checksum_sha1)
            .set_checksum_sha256(self.checksum_sha256)
            .set_expires(self.expires.map(DateTime::from))
            .set_grant_full_control(self.grant_full_control)
            .set_grant_read(self.grant_read)
            .set_grant_read_acp(self.grant_read_acp)
            .set_grant_write_acp(self.grant_write_acp)
            .set_key(self.key)
            .set_metadata(self.metadata)
            .set_server_side_encryption(self.server_side_encryption.map(Into::into))
            .set_storage_class(self.storage_class.map(Into::into))
            .set_website_redirect_location(self.website_redirect_location)
            .set_sse_customer_algorithm(self.sse_customer_algorithm)
            .set_sse_customer_key(self.sse_customer_key)
            .set_sse_customer_key_md5(self.sse_customer_key_md5)
            .set_ssekms_key_id(self.ssekms_key_id)
            .set_ssekms_encryption_context(self.ssekms_encryption_context)
            .set_request_payer(self.request_payer.map(Into::into))
            .set_tagging(self.tagging)
            .set_object_lock_mode(self.object_lock_mode.map(Into::into))
            .set_object_lock_retain_until_date(
                self.object_lock_retain_until_date.map(DateTime::from),
            )
            .set_object_lock_legal_hold_status(self.object_lock_legal_hold_status.map(Into::into))
            .set_expected_bucket_owner(self.expected_bucket_owner))
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableGetObjectOutput {
    pub key: Option<String>,
    pub bucket: Option<String>,
    pub body: Vec<u8>,
    pub delete_marker: Option<bool>,
    pub accept_ranges: Option<String>,
    pub expiration: Option<String>,
    pub restore: Option<String>,
    pub last_modified: Option<SystemTime>,
    pub content_length: Option<i64>,
    pub e_tag: Option<String>,
    pub checksum_crc32: Option<String>,
    pub checksum_crc32_c: Option<String>,
    pub checksum_sha1: Option<String>,
    pub checksum_sha256: Option<String>,
    pub missing_meta: Option<i32>,
    pub version_id: Option<String>,
    pub cache_control: Option<String>,
    pub content_disposition: Option<String>,
    pub content_encoding: Option<String>,
    pub content_language: Option<String>,
    pub content_range: Option<String>,
    pub content_type: Option<String>,
    pub expires: Option<SystemTime>,
    pub website_redirect_location: Option<String>,
    pub server_side_encryption: Option<SerializableServerSideEncryption>,
    pub metadata: Option<HashMap<String, String>>,
    pub sse_customer_algorithm: Option<String>,
    pub sse_customer_key_md5: Option<String>,
    pub ssekms_key_id: Option<String>,
    pub bucket_key_enabled: Option<bool>,
    pub storage_class: Option<SerializableStorageClass>,
    pub request_charged: Option<SerializableS3RequestCharged>,
    pub replication_status: Option<SerializableReplicationStatus>,
    pub parts_count: Option<i32>,
    pub tag_count: Option<i32>,
    pub object_lock_mode: Option<SerializableObjectLockMode>,
    pub object_lock_retain_until_date: Option<SystemTime>,
    pub object_lock_legal_hold_status: Option<SerializableObjectLockLegalHoldStatus>,
}

#[allow(deprecated)]
impl From<GetObjectOutput> for SerializableGetObjectOutput {
    fn from(output: GetObjectOutput) -> Self {
        Self {
            key: None,
            bucket: None,
            body: output
                .body
                .into_inner()
                .bytes()
                .map(|b| b.into())
                .unwrap_or_default(),
            delete_marker: output.delete_marker,
            accept_ranges: output.accept_ranges,
            expiration: output.expiration,
            restore: output.restore,
            last_modified: output
                .last_modified
                .map(|t| UNIX_EPOCH + Duration::new(t.secs() as u64, t.subsec_nanos())),
            content_length: output.content_length,
            e_tag: output.e_tag,
            checksum_crc32: output.checksum_crc32,
            checksum_crc32_c: output.checksum_crc32_c,
            checksum_sha1: output.checksum_sha1,
            checksum_sha256: output.checksum_sha256,
            missing_meta: output.missing_meta,
            version_id: output.version_id,
            cache_control: output.cache_control,
            content_disposition: output.content_disposition,
            content_encoding: output.content_encoding,
            content_language: output.content_language,
            content_range: output.content_range,
            content_type: output.content_type,
            expires: output
                .expires
                .map(|t| UNIX_EPOCH + Duration::new(t.secs() as u64, t.subsec_nanos())),
            website_redirect_location: output.website_redirect_location,
            server_side_encryption: output.server_side_encryption.map(Into::into),
            metadata: output.metadata,
            sse_customer_algorithm: output.sse_customer_algorithm,
            sse_customer_key_md5: output.sse_customer_key_md5,
            ssekms_key_id: output.ssekms_key_id,
            bucket_key_enabled: output.bucket_key_enabled,
            storage_class: output.storage_class.map(Into::into),
            request_charged: output.request_charged.map(Into::into),
            replication_status: output.replication_status.map(Into::into),
            parts_count: output.parts_count,
            tag_count: output.tag_count,
            object_lock_mode: output.object_lock_mode.map(Into::into),
            object_lock_retain_until_date: output
                .object_lock_retain_until_date
                .map(|t| UNIX_EPOCH + Duration::new(t.secs() as u64, t.subsec_nanos())),
            object_lock_legal_hold_status: output.object_lock_legal_hold_status.map(Into::into),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum SerializableReplicationStatus {
    Complete,
    Completed,
    Failed,
    Pending,
    Replica,
    Unknown,
}

impl From<ReplicationStatus> for SerializableReplicationStatus {
    fn from(status: ReplicationStatus) -> Self {
        match status {
            ReplicationStatus::Complete => SerializableReplicationStatus::Complete,
            ReplicationStatus::Completed => SerializableReplicationStatus::Completed,
            ReplicationStatus::Failed => SerializableReplicationStatus::Failed,
            ReplicationStatus::Pending => SerializableReplicationStatus::Pending,
            ReplicationStatus::Replica => SerializableReplicationStatus::Replica,
            _ => SerializableReplicationStatus::Unknown,
        }
    }
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum SerializableS3RequestCharged {
    Requester,
    Unknown,
}
impl From<RequestCharged> for SerializableS3RequestCharged {
    fn from(charged: RequestCharged) -> Self {
        match charged {
            RequestCharged::Requester => SerializableS3RequestCharged::Requester,
            _ => SerializableS3RequestCharged::Unknown,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct SerializablePutObjectInput {
    pub acl: Option<SerializableObjectCannedAcl>,
    pub body: Vec<u8>,
    pub bucket: Option<String>,
    pub cache_control: Option<String>,
    pub content_disposition: Option<String>,
    pub content_encoding: Option<String>,
    pub content_language: Option<String>,
    pub content_length: Option<i64>,
    pub content_md5: Option<String>,
    pub content_type: Option<String>,
    pub checksum_algorithm: Option<SerializableChecksumAlgorithm>,
    pub checksum_crc32: Option<String>,
    pub checksum_crc32_c: Option<String>,
    pub checksum_sha1: Option<String>,
    pub checksum_sha256: Option<String>,
    pub expires: Option<SystemTime>,
    pub grant_full_control: Option<String>,
    pub grant_read: Option<String>,
    pub grant_read_acp: Option<String>,
    pub grant_write_acp: Option<String>,
    pub key: Option<String>,
    pub metadata: Option<HashMap<String, String>>,
    pub server_side_encryption: Option<SerializableServerSideEncryption>,
    pub storage_class: Option<SerializableStorageClass>,
    pub website_redirect_location: Option<String>,
    pub sse_customer_algorithm: Option<String>,
    pub sse_customer_key: Option<String>,
    pub sse_customer_key_md5: Option<String>,
    pub ssekms_key_id: Option<String>,
    pub ssekms_encryption_context: Option<String>,
    pub bucket_key_enabled: Option<bool>,
    pub request_payer: Option<SerializableRequestPayer>,
    pub tagging: Option<String>,
    pub object_lock_mode: Option<SerializableObjectLockMode>,
    pub object_lock_retain_until_date: Option<SystemTime>,
    pub object_lock_legal_hold_status: Option<SerializableObjectLockLegalHoldStatus>,
    pub expected_bucket_owner: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum SerializableObjectLockLegalHoldStatus {
    Off,
    On,
    Unknown,
}
impl From<SerializableObjectLockLegalHoldStatus> for ObjectLockLegalHoldStatus {
    fn from(status: SerializableObjectLockLegalHoldStatus) -> Self {
        match status {
            SerializableObjectLockLegalHoldStatus::Off => ObjectLockLegalHoldStatus::Off,
            SerializableObjectLockLegalHoldStatus::On => ObjectLockLegalHoldStatus::On,
            SerializableObjectLockLegalHoldStatus::Unknown => ObjectLockLegalHoldStatus::Off,
        }
    }
}
impl From<ObjectLockLegalHoldStatus> for SerializableObjectLockLegalHoldStatus {
    fn from(status: ObjectLockLegalHoldStatus) -> Self {
        match status {
            ObjectLockLegalHoldStatus::Off => SerializableObjectLockLegalHoldStatus::Off,
            ObjectLockLegalHoldStatus::On => SerializableObjectLockLegalHoldStatus::On,
            _ => SerializableObjectLockLegalHoldStatus::Unknown,
        }
    }
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum SerializableObjectLockMode {
    Compliance,
    Governance,
    Unknown,
}

impl From<SerializableObjectLockMode> for ObjectLockMode {
    fn from(mode: SerializableObjectLockMode) -> Self {
        match mode {
            SerializableObjectLockMode::Compliance => ObjectLockMode::Compliance,
            SerializableObjectLockMode::Governance => ObjectLockMode::Governance,
            SerializableObjectLockMode::Unknown => ObjectLockMode::Governance,
        }
    }
}
impl From<ObjectLockMode> for SerializableObjectLockMode {
    fn from(mode: ObjectLockMode) -> Self {
        match mode {
            ObjectLockMode::Compliance => SerializableObjectLockMode::Compliance,
            ObjectLockMode::Governance => SerializableObjectLockMode::Governance,
            _ => SerializableObjectLockMode::Unknown,
        }
    }
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum SerializableRequestPayer {
    Requester,
    Unknown,
}
impl From<SerializableRequestPayer> for RequestPayer {
    fn from(payer: SerializableRequestPayer) -> Self {
        match payer {
            SerializableRequestPayer::Requester => RequestPayer::Requester,
            SerializableRequestPayer::Unknown => RequestPayer::Requester,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum SerializableObjectCannedAcl {
    AuthenticatedRead,
    AwsExecRead,
    BucketOwnerFullControl,
    BucketOwnerRead,
    Private,
    PublicRead,
    PublicReadWrite,
    Unknown,
}
impl From<SerializableObjectCannedAcl> for ObjectCannedAcl {
    fn from(acl: SerializableObjectCannedAcl) -> Self {
        match acl {
            SerializableObjectCannedAcl::AuthenticatedRead => ObjectCannedAcl::AuthenticatedRead,
            SerializableObjectCannedAcl::AwsExecRead => ObjectCannedAcl::AwsExecRead,
            SerializableObjectCannedAcl::BucketOwnerFullControl => {
                ObjectCannedAcl::BucketOwnerFullControl
            }
            SerializableObjectCannedAcl::BucketOwnerRead => ObjectCannedAcl::BucketOwnerRead,
            SerializableObjectCannedAcl::Private => ObjectCannedAcl::Private,
            SerializableObjectCannedAcl::PublicRead => ObjectCannedAcl::PublicRead,
            SerializableObjectCannedAcl::PublicReadWrite => ObjectCannedAcl::PublicReadWrite,
            SerializableObjectCannedAcl::Unknown => ObjectCannedAcl::Private,
        }
    }
}
