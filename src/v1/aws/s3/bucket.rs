use anyhow::Context;
use aws_sdk_s3::{
    operation::{
        create_bucket::CreateBucketOutput,
        get_bucket_accelerate_configuration::GetBucketAccelerateConfigurationOutput,
        get_bucket_acl::GetBucketAclOutput,
        get_bucket_analytics_configuration::GetBucketAnalyticsConfigurationOutput,
        get_bucket_cors::GetBucketCorsOutput, get_bucket_encryption::GetBucketEncryptionOutput,
        get_bucket_intelligent_tiering_configuration::GetBucketIntelligentTieringConfigurationOutput,
        get_bucket_inventory_configuration::GetBucketInventoryConfigurationOutput,
        get_bucket_lifecycle_configuration::GetBucketLifecycleConfigurationOutput,
        get_bucket_location::GetBucketLocationOutput, get_bucket_logging::GetBucketLoggingOutput,
        get_bucket_metrics_configuration::GetBucketMetricsConfigurationOutput,
        get_bucket_notification_configuration::GetBucketNotificationConfigurationOutput,
        get_bucket_ownership_controls::GetBucketOwnershipControlsOutput,
        get_bucket_policy::GetBucketPolicyOutput,
        get_bucket_policy_status::GetBucketPolicyStatusOutput,
        get_bucket_replication::GetBucketReplicationOutput,
        get_bucket_request_payment::GetBucketRequestPaymentOutput,
        get_bucket_tagging::GetBucketTaggingOutput,
        get_bucket_versioning::GetBucketVersioningOutput,
        get_bucket_website::GetBucketWebsiteOutput,
        put_bucket_accelerate_configuration::PutBucketAccelerateConfigurationOutput,
        put_bucket_acl::PutBucketAclOutput,
        put_bucket_analytics_configuration::PutBucketAnalyticsConfigurationOutput,
        put_bucket_cors::PutBucketCorsOutput, put_bucket_encryption::PutBucketEncryptionOutput,
        put_bucket_intelligent_tiering_configuration::PutBucketIntelligentTieringConfigurationOutput,
        put_bucket_inventory_configuration::PutBucketInventoryConfigurationOutput,
        put_bucket_lifecycle_configuration::PutBucketLifecycleConfigurationOutput,
        put_bucket_logging::PutBucketLoggingOutput,
        put_bucket_metrics_configuration::PutBucketMetricsConfigurationOutput,
        put_bucket_notification_configuration::PutBucketNotificationConfigurationOutput,
        put_bucket_ownership_controls::PutBucketOwnershipControlsOutput,
        put_bucket_policy::PutBucketPolicyOutput,
        put_bucket_replication::PutBucketReplicationOutput,
        put_bucket_request_payment::PutBucketRequestPaymentOutput,
        put_bucket_tagging::PutBucketTaggingOutput,
        put_bucket_versioning::PutBucketVersioningOutput,
        put_bucket_website::PutBucketWebsiteOutput,
    },
    primitives::DateTime,
    types::{
        builders::{
            AbortIncompleteMultipartUploadBuilder, AccelerateConfigurationBuilder,
            AccessControlPolicyBuilder, AccessControlTranslationBuilder,
            AnalyticsAndOperatorBuilder, AnalyticsConfigurationBuilder,
            AnalyticsExportDestinationBuilder, AnalyticsS3BucketDestinationBuilder,
            BucketInfoBuilder, BucketLifecycleConfigurationBuilder, BucketLoggingStatusBuilder,
            ConditionBuilder, CorsConfigurationBuilder, CorsRuleBuilder,
            CreateBucketConfigurationBuilder, DeleteMarkerReplicationBuilder, DestinationBuilder,
            EncryptionConfigurationBuilder, ErrorDocumentBuilder, EventBridgeConfigurationBuilder,
            ExistingObjectReplicationBuilder, FilterRuleBuilder, GrantBuilder, GranteeBuilder,
            IndexDocumentBuilder, IntelligentTieringAndOperatorBuilder,
            IntelligentTieringConfigurationBuilder, IntelligentTieringFilterBuilder,
            InventoryConfigurationBuilder, InventoryDestinationBuilder, InventoryEncryptionBuilder,
            InventoryFilterBuilder, InventoryS3BucketDestinationBuilder, InventoryScheduleBuilder,
            LambdaFunctionConfigurationBuilder, LifecycleExpirationBuilder,
            LifecycleRuleAndOperatorBuilder, LifecycleRuleBuilder, LocationInfoBuilder,
            LoggingEnabledBuilder, MetricsAndOperatorBuilder, MetricsBuilder,
            MetricsConfigurationBuilder, NoncurrentVersionExpirationBuilder,
            NoncurrentVersionTransitionBuilder, NotificationConfigurationBuilder,
            NotificationConfigurationFilterBuilder, OwnerBuilder, OwnershipControlsBuilder,
            OwnershipControlsRuleBuilder, PartitionedPrefixBuilder, PolicyStatusBuilder,
            QueueConfigurationBuilder, RedirectAllRequestsToBuilder, RedirectBuilder,
            ReplicaModificationsBuilder, ReplicationConfigurationBuilder,
            ReplicationRuleAndOperatorBuilder, ReplicationRuleBuilder, ReplicationTimeBuilder,
            ReplicationTimeValueBuilder, RequestPaymentConfigurationBuilder, RoutingRuleBuilder,
            S3KeyFilterBuilder, ServerSideEncryptionByDefaultBuilder,
            ServerSideEncryptionConfigurationBuilder, ServerSideEncryptionRuleBuilder,
            SimplePrefixBuilder, SourceSelectionCriteriaBuilder, SseKmsEncryptedObjectsBuilder,
            SsekmsBuilder, Sses3Builder, StorageClassAnalysisBuilder,
            StorageClassAnalysisDataExportBuilder, TagBuilder, TaggingBuilder, TargetGrantBuilder,
            TargetObjectKeyFormatBuilder, TieringBuilder, TopicConfigurationBuilder,
            TransitionBuilder, VersioningConfigurationBuilder, WebsiteConfigurationBuilder,
        },
        *,
    },
    Client,
};
use serde::{Deserialize, Serialize};

use crate::{
    prelude::{
        ArnProvider, AwsManager, AwsResource, AwsResourceCreator, Policy, ResourceState,
        SerializableTag,
    },
    v1::{
        manager::{ManagerError, ResourceManager},
        plan::ResourceItem,
    },
};

pub type BucketInput = SerializableBucketInput;
pub type BucketOutput = SerializableBucketOutput;
pub type BucketManager = AwsManager<BucketInput, BucketOutput, Client>;
pub type Bucket<'a> = AwsResource<'a, BucketInput, BucketOutput>;

impl<'a> Bucket<'a> {
    pub fn allow_policy(&self) -> anyhow::Result<Policy> {
        let id = format!("{}-allow", self.inner.name());
        let policy = self
            .aws
            .resource::<Policy>(&id, ResourceState::Present, Default::default())
            .context("Error creating policy")?;
        policy
            .configure(
                "Allow",
                vec![
                    "s3:PutObject",
                    "s3:PutObjectAcl",
                    "s3:GetObject",
                    "s3:ListBucket",
                ],
                self,
            )
            .context("Could not configure policy")?;
        Ok(policy)
    }
}

impl ArnProvider for BucketOutput {
    fn arn(&self) -> Option<Vec<String>> {
        Some(vec![
            format!("arn:aws:s3:::{}", self.bucket_name),
            format!("arn:aws:s3:::{}/*", self.bucket_name),
        ])
    }
}
impl AwsResourceCreator for Bucket<'_> {
    type Input = BucketInput;
    type Output = BucketOutput;
    fn r#type() -> crate::prelude::AwsType {
        crate::prelude::AwsType::Bucket
    }
    fn manager(
        handle: &tokio::runtime::Handle,
        config: &aws_config::SdkConfig,
    ) -> std::sync::Arc<dyn ResourceManager<Self::Input, Self::Output>> {
        BucketManager::new(handle, Client::new(config), config).arc()
    }
    fn input_hook(_id: &str, _input: &mut Self::Input) {}
}

impl ResourceManager<BucketInput, BucketOutput> for BucketManager {
    fn lookup(
        &self,
        latest: &BucketOutput,
    ) -> Result<Option<BucketOutput>, crate::v1::manager::ManagerError> {
        self.lookup(&latest.bucket_name).or_else(|e| match e {
            ManagerError::LookupFail(ref msg) if msg.contains("NoSuchBucket") => Ok(None),
            _ => Err(e),
        })
    }

    fn lookup_by_input(
        &self,
        input: &BucketInput,
    ) -> Result<Option<BucketOutput>, crate::v1::manager::ManagerError> {
        self.lookup(input.bucket.bucket.as_ref().unwrap())
            .or_else(|e| match e {
                ManagerError::LookupFail(ref msg) if msg.contains("NoSuchBucket") => Ok(None),
                _ => Err(e),
            })
    }

    fn create(
        &self,
        input: &mut BucketInput,
    ) -> Result<BucketOutput, crate::v1::manager::ManagerError> {
        self.create(input)
    }

    fn delete(&self, latest: &BucketOutput) -> Result<bool, crate::v1::manager::ManagerError> {
        self.delete_bucket(&latest.bucket_name)
    }

    fn syncup(
        &self,
        _latest: &BucketOutput,
        _input: &mut BucketInput,
    ) -> Result<Option<BucketOutput>, crate::v1::manager::ManagerError> {
        Ok(None)
    }
}

impl BucketManager {
    fn delete_bucket(&self, bucket_name: &str) -> Result<bool, ManagerError> {
        self.handle.block_on(async {
            self.client
                .delete_bucket()
                .bucket(bucket_name)
                .send()
                .await
                .map(|_| true)
                .map_err(|e| ManagerError::DeleteFail(format!("{:?}", e.into_source())))
        })
    }

    fn lookup(
        &self,
        bucket_name: &str,
    ) -> Result<Option<BucketOutput>, crate::v1::manager::ManagerError> {
        let accelerate_configuration = self.get_accelerate_configuration(bucket_name)?;
        let acl = self.get_acl(bucket_name)?;
        let cors = self.get_cors(bucket_name)?;
        let encryption = self.get_encryption(bucket_name)?;
        let lifecycle_configuration = self.get_lifecycle_configuration(bucket_name)?;
        let location = self.get_location(bucket_name)?;
        let logging = self.get_logging(bucket_name)?;
        let notification_configuration = self.get_notification_configuration(bucket_name)?;
        let ownership_controls = self.get_ownership_controls(bucket_name)?;
        let policy = self.get_policy(bucket_name)?;
        let replication = self.get_replication(bucket_name)?;
        let request_payment = self.get_request_payment(bucket_name)?;
        let tagging = self.get_tagging(bucket_name)?;
        let versioning = self.get_versioning(bucket_name)?;
        let website = self.get_website(bucket_name)?;
        let policy_status = self.get_policy_status(bucket_name)?;
        Ok(Some(BucketOutput {
            bucket_name: bucket_name.to_string(),
            accelerate_configuration,
            acl,
            cors,
            encryption,
            lifecycle_configuration,
            location,
            logging,
            notification_configuration,
            ownership_controls,
            policy,
            replication,
            request_payment,
            tagging,
            versioning,
            website,
            policy_status,
        }))
    }

    #[allow(clippy::assigning_clones)]
    pub fn create(
        &self,
        input: &mut BucketInput,
    ) -> Result<BucketOutput, crate::v1::manager::ManagerError> {
        if input.bucket.bucket.is_none() {
            return Err(ManagerError::CreateFail(
                "bucket_name is not present".to_string(),
            ));
        };
        if let Err(e) = self.create_bucket_input(input.bucket.clone()) {
            match e {
                ManagerError::CreateFail(ref msg) if msg.contains("BucketAlreadyOwnedByYou") => (),
                _ => return Err(e),
            }
        }
        if let Some(accelerate_configuration) = input.accelerate_configuration.as_mut() {
            accelerate_configuration.bucket = input.bucket.bucket.clone();
            self.put_bucket_accelerate_configuration(accelerate_configuration.clone())?;
        }
        if let Some(metrics_configuration) = input.metrics_configuration.as_mut() {
            metrics_configuration.bucket = input.bucket.bucket.clone();
            self.put_bucket_metrics_configuration(metrics_configuration.clone())?;
        }
        if let Some(acl) = input.acl.as_mut() {
            acl.bucket = input.bucket.bucket.clone();
            self.put_bucket_acl(acl.clone())?;
        }
        if let Some(analytics_configuration) = input.analytics_configuration.as_mut() {
            analytics_configuration.bucket = input.bucket.bucket.clone();
            self.put_bucket_analytics_configuration(analytics_configuration.clone())?;
        }
        if let Some(cors) = input.cors.as_mut() {
            cors.bucket = input.bucket.bucket.clone();
            self.put_bucket_cors(cors.clone())?;
        }
        if let Some(encryption) = input.encryption.as_mut() {
            encryption.bucket = input.bucket.bucket.clone();
            self.put_bucket_encryption(encryption.clone())?;
        }
        if let Some(intelligent_tiering_configuration) =
            input.intelligent_tiering_configuration.as_mut()
        {
            intelligent_tiering_configuration.bucket = input.bucket.bucket.clone();
            self.put_bucket_intelligent_tiering_configuration(
                intelligent_tiering_configuration.clone(),
            )?;
        }
        if let Some(inventory_configuration) = input.inventory_configuration.as_mut() {
            inventory_configuration.bucket = input.bucket.bucket.clone();
            self.put_bucket_inventory_configuration(inventory_configuration.clone())?;
        }
        if let Some(lifecycle_configuration) = input.lifecycle_configuration.as_mut() {
            lifecycle_configuration.bucket = input.bucket.bucket.clone();
            self.put_bucket_lifecycle_configuration(lifecycle_configuration.clone())?;
        }
        if let Some(logging) = input.logging.as_mut() {
            logging.bucket = input.bucket.bucket.clone();
            self.put_bucket_logging(logging.clone())?;
        }
        if let Some(notification_configuration) = input.notification_configuration.as_mut() {
            notification_configuration.bucket = input.bucket.bucket.clone();
            self.put_bucket_notification_configuration(notification_configuration.clone())?;
        }
        if let Some(ownership_controls) = input.ownership_controls.as_mut() {
            ownership_controls.bucket = input.bucket.bucket.clone();
            self.put_bucket_ownership_controls(ownership_controls.clone())?;
        }
        if let Some(policy) = input.policy.as_mut() {
            policy.bucket = input.bucket.bucket.clone();
            self.put_bucket_policy(policy.clone())?;
        }
        if let Some(replication) = input.replication.as_mut() {
            replication.bucket = input.bucket.bucket.clone();
            self.put_bucket_replication(replication.clone())?;
        }
        if let Some(request_payment) = input.request_payment.as_mut() {
            request_payment.bucket = input.bucket.bucket.clone();
            self.put_bucket_request_payment(request_payment.clone())?;
        }
        if let Some(tagging) = input.tagging.as_mut() {
            tagging.bucket = input.bucket.bucket.clone();
            self.put_bucket_tagging(tagging.clone())?;
        }
        if let Some(versioning) = input.versioning.as_mut() {
            versioning.bucket = input.bucket.bucket.clone();
            self.put_bucket_versioning(versioning.clone())?;
        }

        if let Some(website) = input.website.as_mut() {
            website.bucket = input.bucket.bucket.clone();
            self.put_bucket_website(website.clone())?;
        }

        let bucket = self.lookup(input.bucket.bucket.as_ref().unwrap())?;
        if let Some(bucket) = bucket {
            Ok(bucket)
        } else {
            Err(ManagerError::CreateFail(
                "Could not retrieve recently created bucket".to_string(),
            ))
        }
    }

    fn create_bucket_input(
        &self,
        input: SerializableCreateBucketInput,
    ) -> Result<CreateBucketOutput, ManagerError> {
        self.handle.block_on(async {
            self.client
                .create_bucket()
                .set_bucket(input.bucket)
                .set_create_bucket_configuration(
                    input
                        .create_bucket_configuration
                        .map(CreateBucketConfiguration::from),
                )
                .set_acl(input.acl.map(BucketCannedAcl::from))
                .set_grant_full_control(input.grant_full_control)
                .set_grant_read(input.grant_read)
                .set_grant_read_acp(input.grant_read_acp)
                .set_grant_write(input.grant_write)
                .set_grant_write_acp(input.grant_write_acp)
                .send()
                .await
                .map_err(|e| ManagerError::CreateFail(format!("{:?}", e)))
        })
    }
    fn put_bucket_accelerate_configuration(
        &self,
        input: SerializablePutBucketAccelerateConfigurationInput,
    ) -> Result<PutBucketAccelerateConfigurationOutput, ManagerError> {
        self.handle.block_on(async {
            self.client
                .put_bucket_accelerate_configuration()
                .set_bucket(input.bucket)
                .set_accelerate_configuration(
                    input
                        .accelerate_configuration
                        .clone()
                        .map(AccelerateConfiguration::from),
                )
                .set_checksum_algorithm(input.checksum_algorithm.map(ChecksumAlgorithm::from))
                .send()
                .await
                .map_err(|e| {
                    crate::v1::manager::ManagerError::CreateFail(format!("{:?}", e.into_source()))
                })
        })
    }
    fn put_bucket_acl(
        &self,
        input: SerializablePutBucketAclInput,
    ) -> Result<PutBucketAclOutput, ManagerError> {
        self.handle.block_on(async {
            self.client
                .put_bucket_acl()
                .set_bucket(input.bucket)
                .set_access_control_policy(
                    input
                        .access_control_policy
                        .clone()
                        .map(AccessControlPolicy::from),
                )
                .set_grant_full_control(input.grant_full_control)
                .set_grant_read(input.grant_read)
                .set_grant_read_acp(input.grant_read_acp)
                .set_grant_write(input.grant_write)
                .set_grant_write_acp(input.grant_write_acp)
                .set_checksum_algorithm(input.checksum_algorithm.map(ChecksumAlgorithm::from))
                .set_content_md5(input.content_md5)
                .send()
                .await
                .map_err(|e| {
                    crate::v1::manager::ManagerError::CreateFail(format!("{:?}", e.into_source()))
                })
        })
    }
    fn put_bucket_analytics_configuration(
        &self,
        input: SerializablePutBucketAnalyticsConfigurationInput,
    ) -> Result<PutBucketAnalyticsConfigurationOutput, ManagerError> {
        self.handle.block_on(async {
            self.client
                .put_bucket_analytics_configuration()
                .set_bucket(input.bucket)
                .set_analytics_configuration(
                    input
                        .analytics_configuration
                        .clone()
                        .map(AnalyticsConfiguration::from),
                )
                .set_id(input.id.clone())
                .send()
                .await
                .map_err(|e| {
                    crate::v1::manager::ManagerError::CreateFail(format!("{:?}", e.into_source()))
                })
        })
    }
    fn put_bucket_cors(
        &self,
        input: SerializablePutBucketCorsInput,
    ) -> Result<PutBucketCorsOutput, ManagerError> {
        self.handle.block_on(async {
            self.client
                .put_bucket_cors()
                .set_bucket(input.bucket)
                .set_cors_configuration(
                    input
                        .cors_configuration
                        .clone()
                        .map(CorsConfiguration::from),
                )
                .set_checksum_algorithm(input.checksum_algorithm.map(ChecksumAlgorithm::from))
                .set_content_md5(input.content_md5)
                .send()
                .await
                .map_err(|e| {
                    crate::v1::manager::ManagerError::CreateFail(format!("{:?}", e.into_source()))
                })
        })
    }
    fn put_bucket_encryption(
        &self,
        input: SerializablePutBucketEncryptionInput,
    ) -> Result<PutBucketEncryptionOutput, ManagerError> {
        self.handle.block_on(async {
            self.client
                .put_bucket_encryption()
                .set_bucket(input.bucket)
                .set_server_side_encryption_configuration(
                    input
                        .server_side_encryption_configuration
                        .map(ServerSideEncryptionConfiguration::from),
                )
                .set_checksum_algorithm(input.checksum_algorithm.map(ChecksumAlgorithm::from))
                .set_content_md5(input.content_md5)
                .send()
                .await
                .map_err(|e| {
                    crate::v1::manager::ManagerError::CreateFail(format!("{:?}", e.into_source()))
                })
        })
    }
    fn put_bucket_intelligent_tiering_configuration(
        &self,
        input: SerializablePutBucketIntelligentTieringConfigurationInput,
    ) -> Result<PutBucketIntelligentTieringConfigurationOutput, ManagerError> {
        self.handle.block_on(async {
            self.client
                .put_bucket_intelligent_tiering_configuration()
                .set_bucket(input.bucket)
                .set_intelligent_tiering_configuration(
                    input
                        .intelligent_tiering_configuration
                        .map(IntelligentTieringConfiguration::from),
                )
                .set_id(input.id)
                .send()
                .await
                .map_err(|e| {
                    crate::v1::manager::ManagerError::CreateFail(format!("{:?}", e.into_source()))
                })
        })
    }
    fn put_bucket_inventory_configuration(
        &self,
        input: SerializablePutBucketInventoryConfigurationInput,
    ) -> Result<PutBucketInventoryConfigurationOutput, ManagerError> {
        self.handle.block_on(async {
            self.client
                .put_bucket_inventory_configuration()
                .set_bucket(input.bucket)
                .set_inventory_configuration(
                    input
                        .inventory_configuration
                        .map(InventoryConfiguration::from),
                )
                .set_id(input.id)
                .send()
                .await
                .map_err(|e| {
                    crate::v1::manager::ManagerError::CreateFail(format!("{:?}", e.into_source()))
                })
        })
    }
    fn put_bucket_lifecycle_configuration(
        &self,
        input: SerializablePutBucketLifecycleConfigurationInput,
    ) -> Result<PutBucketLifecycleConfigurationOutput, ManagerError> {
        self.handle.block_on(async {
            self.client
                .put_bucket_lifecycle_configuration()
                .set_bucket(input.bucket)
                .set_checksum_algorithm(input.checksum_algorithm.map(ChecksumAlgorithm::from))
                .set_lifecycle_configuration(
                    input
                        .lifecycle_configuration
                        .clone()
                        .map(BucketLifecycleConfiguration::from),
                )
                .send()
                .await
                .map_err(|e| {
                    crate::v1::manager::ManagerError::CreateFail(format!("{:?}", e.into_source()))
                })
        })
    }
    fn put_bucket_logging(
        &self,
        input: SerializablePutBucketLoggingInput,
    ) -> Result<PutBucketLoggingOutput, ManagerError> {
        self.handle.block_on(async {
            self.client
                .put_bucket_logging()
                .set_bucket(input.bucket)
                .set_bucket_logging_status(
                    input.bucket_logging_status.map(BucketLoggingStatus::from),
                )
                .set_checksum_algorithm(input.checksum_algorithm.map(ChecksumAlgorithm::from))
                .set_content_md5(input.content_md5)
                .send()
                .await
                .map_err(|e| {
                    crate::v1::manager::ManagerError::CreateFail(format!("{:?}", e.into_source()))
                })
        })
    }
    fn put_bucket_metrics_configuration(
        &self,
        input: SerializablePutBucketMetricsConfigurationInput,
    ) -> Result<PutBucketMetricsConfigurationOutput, ManagerError> {
        self.handle.block_on(async {
            self.client
                .put_bucket_metrics_configuration()
                .set_bucket(input.bucket)
                .set_metrics_configuration(
                    input.metrics_configuration.map(MetricsConfiguration::from),
                )
                .set_id(input.id)
                .send()
                .await
                .map_err(|e| {
                    crate::v1::manager::ManagerError::CreateFail(format!("{:?}", e.into_source()))
                })
        })
    }
    fn put_bucket_notification_configuration(
        &self,
        input: SerializablePutBucketNotificationConfigurationInput,
    ) -> Result<PutBucketNotificationConfigurationOutput, ManagerError> {
        self.handle.block_on(async {
            self.client
                .put_bucket_notification_configuration()
                .set_bucket(input.bucket)
                .set_notification_configuration(
                    input
                        .notification_configuration
                        .map(NotificationConfiguration::from),
                )
                .send()
                .await
                .map_err(|e| {
                    crate::v1::manager::ManagerError::CreateFail(format!("{:?}", e.into_source()))
                })
        })
    }
    fn put_bucket_ownership_controls(
        &self,
        input: SerializablePutBucketOwnershipControlsInput,
    ) -> Result<PutBucketOwnershipControlsOutput, ManagerError> {
        self.handle.block_on(async {
            self.client
                .put_bucket_ownership_controls()
                .set_bucket(input.bucket)
                .set_ownership_controls(input.ownership_controls.map(OwnershipControls::from))
                .set_content_md5(input.content_md5)
                .send()
                .await
                .map_err(|e| {
                    crate::v1::manager::ManagerError::CreateFail(format!("{:?}", e.into_source()))
                })
        })
    }
    fn put_bucket_policy(
        &self,
        input: SerializablePutBucketPolicyInput,
    ) -> Result<PutBucketPolicyOutput, ManagerError> {
        self.handle.block_on(async {
            self.client
                .put_bucket_policy()
                .set_bucket(input.bucket)
                .set_policy(input.policy)
                .set_checksum_algorithm(input.checksum_algorithm.map(ChecksumAlgorithm::from))
                .set_content_md5(input.content_md5)
                .send()
                .await
                .map_err(|e| {
                    crate::v1::manager::ManagerError::CreateFail(format!("{:?}", e.into_source()))
                })
        })
    }
    fn put_bucket_replication(
        &self,
        input: SerializablePutBucketReplicationInput,
    ) -> Result<PutBucketReplicationOutput, ManagerError> {
        self.handle.block_on(async {
            self.client
                .put_bucket_replication()
                .set_bucket(input.bucket)
                .set_replication_configuration(
                    input
                        .replication_configuration
                        .map(ReplicationConfiguration::from),
                )
                .set_checksum_algorithm(input.checksum_algorithm.map(ChecksumAlgorithm::from))
                .set_content_md5(input.content_md5)
                .send()
                .await
                .map_err(|e| {
                    crate::v1::manager::ManagerError::CreateFail(format!("{:?}", e.into_source()))
                })
        })
    }
    fn put_bucket_request_payment(
        &self,
        input: SerializablePutBucketRequestPaymentInput,
    ) -> Result<PutBucketRequestPaymentOutput, ManagerError> {
        self.handle.block_on(async {
            self.client
                .put_bucket_request_payment()
                .set_bucket(input.bucket)
                .set_request_payment_configuration(
                    input
                        .request_payment_configuration
                        .map(RequestPaymentConfiguration::from),
                )
                .set_checksum_algorithm(input.checksum_algorithm.map(ChecksumAlgorithm::from))
                .set_content_md5(input.content_md5)
                .send()
                .await
                .map_err(|e| {
                    crate::v1::manager::ManagerError::CreateFail(format!("{:?}", e.into_source()))
                })
        })
    }
    fn put_bucket_tagging(
        &self,
        input: SerializablePutBucketTaggingInput,
    ) -> Result<PutBucketTaggingOutput, ManagerError> {
        self.handle.block_on(async {
            self.client
                .put_bucket_tagging()
                .set_bucket(input.bucket)
                .set_tagging(input.tagging.map(Tagging::from))
                .set_checksum_algorithm(input.checksum_algorithm.map(ChecksumAlgorithm::from))
                .set_content_md5(input.content_md5)
                .send()
                .await
                .map_err(|e| {
                    crate::v1::manager::ManagerError::CreateFail(format!("{:?}", e.into_source()))
                })
        })
    }
    fn put_bucket_versioning(
        &self,
        input: SerializablePutBucketVersioningInput,
    ) -> Result<PutBucketVersioningOutput, ManagerError> {
        self.handle.block_on(async {
            self.client
                .put_bucket_versioning()
                .set_bucket(input.bucket)
                .set_versioning_configuration(
                    input
                        .versioning_configuration
                        .map(VersioningConfiguration::from),
                )
                .set_checksum_algorithm(input.checksum_algorithm.map(ChecksumAlgorithm::from))
                .set_content_md5(input.content_md5)
                .send()
                .await
                .map_err(|e| {
                    crate::v1::manager::ManagerError::CreateFail(format!("{:?}", e.into_source()))
                })
        })
    }
    fn put_bucket_website(
        &self,
        input: SerializablePutBucketWebsiteInput,
    ) -> Result<PutBucketWebsiteOutput, ManagerError> {
        self.handle.block_on(async {
            self.client
                .put_bucket_website()
                .set_bucket(input.bucket)
                .set_website_configuration(
                    input.website_configuration.map(WebsiteConfiguration::from),
                )
                .send()
                .await
                .map_err(|e| {
                    crate::v1::manager::ManagerError::CreateFail(format!("{:?}", e.into_source()))
                })
        })
    }

    fn get_accelerate_configuration(
        &self,
        bucket_name: &str,
    ) -> Result<
        Option<SerializableGetBucketAccelerateConfigurationOutput>,
        crate::v1::manager::ManagerError,
    > {
        let resp = self.handle.block_on(async {
            self.client
                .get_bucket_accelerate_configuration()
                .bucket(bucket_name)
                .send()
                .await
                .map_err(|e| {
                    crate::v1::manager::ManagerError::LookupFail(format!("{:?}", e.into_source()))
                })
        })?;
        Ok(Some(resp.into()))
    }
    fn get_acl(
        &self,
        bucket_name: &str,
    ) -> Result<Option<SerializableGetBucketAclOutput>, crate::v1::manager::ManagerError> {
        let resp = self.handle.block_on(async {
            self.client
                .get_bucket_acl()
                .bucket(bucket_name)
                .send()
                .await
                .map_err(|e| {
                    crate::v1::manager::ManagerError::LookupFail(format!("{:?}", e.into_source()))
                })
        })?;
        Ok(Some(resp.into()))
    }
    pub fn get_analytics_configuration(
        &self,
        bucket_name: &str,
    ) -> Result<
        Option<SerializableGetBucketAnalyticsConfigurationOutput>,
        crate::v1::manager::ManagerError,
    > {
        let resp = self.handle.block_on(async {
            self.client
                .get_bucket_analytics_configuration()
                .bucket(bucket_name)
                .send()
                .await
                .map_err(|e| {
                    crate::v1::manager::ManagerError::LookupFail(format!("{:?}", e.into_source()))
                })
        })?;
        Ok(Some(resp.into()))
    }
    fn get_cors(
        &self,
        bucket_name: &str,
    ) -> Result<Option<SerializableGetBucketCorsOutput>, crate::v1::manager::ManagerError> {
        self.handle.block_on(async {
            self.client
                .get_bucket_cors()
                .bucket(bucket_name)
                .send()
                .await
                .map_err(|e| {
                    crate::v1::manager::ManagerError::LookupFail(format!("{:?}", e.into_source()))
                })
                .map(|a| Some(a.into()))
                .or_else(|e| match e.clone() {
                    crate::v1::manager::ManagerError::LookupFail(s) => {
                        if s.contains("NoSuchCORSConfiguration") {
                            Ok(None)
                        } else {
                            Err(e)
                        }
                    }
                    _ => Err(e),
                })
        })
    }
    fn get_encryption(
        &self,
        bucket_name: &str,
    ) -> Result<Option<SerializableGetBucketEncryptionOutput>, crate::v1::manager::ManagerError>
    {
        let resp = self.handle.block_on(async {
            self.client
                .get_bucket_encryption()
                .bucket(bucket_name)
                .send()
                .await
                .map_err(|e| {
                    crate::v1::manager::ManagerError::LookupFail(format!("{:?}", e.into_source()))
                })
        })?;
        Ok(Some(resp.into()))
    }
    fn _get_intelligent_tiering_configuration(
        &self,
        bucket_name: &str,
    ) -> Result<
        Option<SerializableGetBucketIntelligentTieringConfigurationOutput>,
        crate::v1::manager::ManagerError,
    > {
        let resp = self.handle.block_on(async {
            self.client
                .get_bucket_intelligent_tiering_configuration()
                .bucket(bucket_name)
                .send()
                .await
                .map_err(|e| {
                    crate::v1::manager::ManagerError::LookupFail(format!("{:?}", e.into_source()))
                })
        })?;
        Ok(Some(resp.into()))
    }
    pub fn get_inventory_configuration(
        &self,
        bucket_name: &str,
    ) -> Result<
        Option<SerializableGetBucketInventoryConfigurationOutput>,
        crate::v1::manager::ManagerError,
    > {
        let resp = self.handle.block_on(async {
            self.client
                .get_bucket_inventory_configuration()
                .bucket(bucket_name)
                .send()
                .await
                .map_err(|e| {
                    crate::v1::manager::ManagerError::LookupFail(format!("{:?}", e.into_source()))
                })
        })?;
        Ok(Some(resp.into()))
    }
    fn get_lifecycle_configuration(
        &self,
        bucket_name: &str,
    ) -> Result<
        Option<SerializableGetBucketLifecycleConfigurationOutput>,
        crate::v1::manager::ManagerError,
    > {
        self.handle.block_on(async {
            self.client
                .get_bucket_lifecycle_configuration()
                .bucket(bucket_name)
                .send()
                .await
                .map_err(|e| {
                    crate::v1::manager::ManagerError::LookupFail(format!("{:?}", e.into_source()))
                })
                .map(|resp| Some(resp.into()))
                .or_else(|e| match e.clone() {
                    crate::v1::manager::ManagerError::LookupFail(s) => {
                        if s.contains("NoSuchLifecycleConfiguration") {
                            Ok(None)
                        } else {
                            Err(e)
                        }
                    }
                    _ => Err(e),
                })
        })
    }
    fn get_location(
        &self,
        bucket_name: &str,
    ) -> Result<Option<SerializableGetBucketLocationOutput>, crate::v1::manager::ManagerError> {
        let resp = self.handle.block_on(async {
            self.client
                .get_bucket_location()
                .bucket(bucket_name)
                .send()
                .await
                .map_err(|e| {
                    crate::v1::manager::ManagerError::LookupFail(format!("{:?}", e.into_source()))
                })
        })?;
        Ok(Some(resp.into()))
    }
    fn get_logging(
        &self,
        bucket_name: &str,
    ) -> Result<Option<SerializableGetBucketLoggingOutput>, crate::v1::manager::ManagerError> {
        let resp = self.handle.block_on(async {
            self.client
                .get_bucket_logging()
                .bucket(bucket_name)
                .send()
                .await
                .map_err(|e| {
                    crate::v1::manager::ManagerError::LookupFail(format!("{:?}", e.into_source()))
                })
        })?;
        Ok(Some(resp.into()))
    }
    pub fn get_metrics_configuration(
        &self,
        bucket_name: &str,
    ) -> Result<
        Option<SerializableGetBucketMetricsConfigurationOutput>,
        crate::v1::manager::ManagerError,
    > {
        let resp = self.handle.block_on(async {
            self.client
                .get_bucket_metrics_configuration()
                .bucket(bucket_name)
                .send()
                .await
                .map_err(|e| {
                    crate::v1::manager::ManagerError::LookupFail(format!("{:?}", e.into_source()))
                })
        })?;
        Ok(Some(resp.into()))
    }
    fn get_notification_configuration(
        &self,
        bucket_name: &str,
    ) -> Result<
        Option<SerializableGetBucketNotificationConfigurationOutput>,
        crate::v1::manager::ManagerError,
    > {
        let resp = self.handle.block_on(async {
            self.client
                .get_bucket_notification_configuration()
                .bucket(bucket_name)
                .send()
                .await
                .map_err(|e| {
                    crate::v1::manager::ManagerError::LookupFail(format!("{:?}", e.into_source()))
                })
        })?;
        Ok(Some(resp.into()))
    }
    fn get_ownership_controls(
        &self,
        bucket_name: &str,
    ) -> Result<
        Option<SerializableGetBucketOwnershipControlsOutput>,
        crate::v1::manager::ManagerError,
    > {
        let resp = self.handle.block_on(async {
            self.client
                .get_bucket_ownership_controls()
                .bucket(bucket_name)
                .send()
                .await
                .map_err(|e| {
                    crate::v1::manager::ManagerError::LookupFail(format!("{:?}", e.into_source()))
                })
        })?;
        Ok(Some(resp.into()))
    }
    fn get_policy(
        &self,
        bucket_name: &str,
    ) -> Result<Option<SerializableGetBucketPolicyOutput>, crate::v1::manager::ManagerError> {
        self.handle.block_on(async {
            self.client
                .get_bucket_policy()
                .bucket(bucket_name)
                .send()
                .await
                .map_err(|e| {
                    crate::v1::manager::ManagerError::LookupFail(format!("{:?}", e.into_source()))
                })
                .map(|resp| Some(resp.into()))
                .or_else(|e| match e.clone() {
                    crate::v1::manager::ManagerError::LookupFail(s) => {
                        if s.contains("NoSuchBucketPolicy") {
                            Ok(None)
                        } else {
                            Err(e)
                        }
                    }
                    _ => Err(e),
                })
        })
    }
    fn get_policy_status(
        &self,
        bucket_name: &str,
    ) -> Result<Option<SerializableGetBucketPolicyStatusOutput>, crate::v1::manager::ManagerError>
    {
        self.handle.block_on(async {
            self.client
                .get_bucket_policy_status()
                .bucket(bucket_name)
                .send()
                .await
                .map_err(|e| {
                    crate::v1::manager::ManagerError::LookupFail(format!("{:?}", e.into_source()))
                })
                .map(|resp| Some(resp.into()))
                .or_else(|e| match e.clone() {
                    crate::v1::manager::ManagerError::LookupFail(s) => {
                        if s.contains("NoSuchBucketPolicy") {
                            Ok(None)
                        } else {
                            Err(e)
                        }
                    }
                    _ => Err(e),
                })
        })
    }
    fn get_replication(
        &self,
        bucket_name: &str,
    ) -> Result<Option<SerializableGetBucketReplicationOutput>, crate::v1::manager::ManagerError>
    {
        self.handle.block_on(async {
            self.client
                .get_bucket_replication()
                .bucket(bucket_name)
                .send()
                .await
                .map_err(|e| {
                    crate::v1::manager::ManagerError::LookupFail(format!("{:?}", e.into_source()))
                })
                .map(|resp| Some(resp.into()))
                .or_else(|e| match e.clone() {
                    crate::v1::manager::ManagerError::LookupFail(s) => {
                        if s.contains("NotFound") {
                            Ok(None)
                        } else {
                            Err(e)
                        }
                    }
                    _ => Err(e),
                })
        })
    }
    fn get_request_payment(
        &self,
        bucket_name: &str,
    ) -> Result<Option<SerializableGetBucketRequestPaymentOutput>, crate::v1::manager::ManagerError>
    {
        let resp = self.handle.block_on(async {
            self.client
                .get_bucket_request_payment()
                .bucket(bucket_name)
                .send()
                .await
                .map_err(|e| {
                    crate::v1::manager::ManagerError::LookupFail(format!("{:?}", e.into_source()))
                })
        })?;
        Ok(Some(resp.into()))
    }
    fn get_tagging(
        &self,
        bucket_name: &str,
    ) -> Result<Option<SerializableGetBucketTaggingOutput>, crate::v1::manager::ManagerError> {
        self.handle.block_on(async {
            self.client
                .get_bucket_tagging()
                .bucket(bucket_name)
                .send()
                .await
                .map_err(|e| {
                    crate::v1::manager::ManagerError::LookupFail(format!("{:?}", e.into_source()))
                })
                .map(|resp| Some(resp.into()))
                .or_else(|e| match e.clone() {
                    crate::v1::manager::ManagerError::LookupFail(s) => {
                        if s.contains("NoSuchTagSet") {
                            Ok(None)
                        } else {
                            Err(e)
                        }
                    }
                    _ => Err(e),
                })
        })
    }
    fn get_versioning(
        &self,
        bucket_name: &str,
    ) -> Result<Option<SerializableGetBucketVersioningOutput>, crate::v1::manager::ManagerError>
    {
        let resp = self.handle.block_on(async {
            self.client
                .get_bucket_versioning()
                .bucket(bucket_name)
                .send()
                .await
                .map_err(|e| {
                    crate::v1::manager::ManagerError::LookupFail(format!("{:?}", e.into_source()))
                })
        })?;
        Ok(Some(resp.into()))
    }
    fn get_website(
        &self,
        bucket_name: &str,
    ) -> Result<Option<SerializableGetBucketWebsiteOutput>, crate::v1::manager::ManagerError> {
        self.handle.block_on(async {
            self.client
                .get_bucket_website()
                .bucket(bucket_name)
                .send()
                .await
                .map_err(|e| {
                    crate::v1::manager::ManagerError::LookupFail(format!("{:?}", e.into_source()))
                })
                .map(|resp| Some(resp.into()))
                .or_else(|e| match e.clone() {
                    crate::v1::manager::ManagerError::LookupFail(s) => {
                        if s.contains("NoSuchWebsiteConfiguration") {
                            Ok(None)
                        } else {
                            Err(e)
                        }
                    }
                    _ => Err(e),
                })
        })
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableBucketOutput {
    pub bucket_name: String,
    pub accelerate_configuration: Option<SerializableGetBucketAccelerateConfigurationOutput>,
    pub acl: Option<SerializableGetBucketAclOutput>,
    //analytics_configuration: Option<SerializableGetBucketAnalyticsConfigurationOutput>,
    pub cors: Option<SerializableGetBucketCorsOutput>,
    pub encryption: Option<SerializableGetBucketEncryptionOutput>,
    //intelligent_tiering_configuration:
    //    Option<SerializableGetBucketIntelligentTieringConfigurationOutput>,
    //inventory_configuration: Option<SerializableGetBucketInventoryConfigurationOutput>,
    pub lifecycle_configuration: Option<SerializableGetBucketLifecycleConfigurationOutput>,
    pub location: Option<SerializableGetBucketLocationOutput>,
    pub logging: Option<SerializableGetBucketLoggingOutput>,
    //metrics_configuration: Option<SerializableGetBucketMetricsConfigurationOutput>,
    pub notification_configuration: Option<SerializableGetBucketNotificationConfigurationOutput>,
    pub ownership_controls: Option<SerializableGetBucketOwnershipControlsOutput>,
    pub policy: Option<SerializableGetBucketPolicyOutput>,
    pub policy_status: Option<SerializableGetBucketPolicyStatusOutput>,
    pub replication: Option<SerializableGetBucketReplicationOutput>,
    pub request_payment: Option<SerializableGetBucketRequestPaymentOutput>,
    pub tagging: Option<SerializableGetBucketTaggingOutput>,
    pub versioning: Option<SerializableGetBucketVersioningOutput>,
    pub website: Option<SerializableGetBucketWebsiteOutput>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableGetBucketWebsiteOutput {
    pub redirect_all_requests_to: Option<SerializableRedirectAllRequestsTo>,
    pub index_document: Option<SerializableIndexDocument>,
    pub error_document: Option<SerializableErrorDocument>,
    pub routing_rules: Option<Vec<SerializableRoutingRule>>,
}
impl From<GetBucketWebsiteOutput> for SerializableGetBucketWebsiteOutput {
    fn from(value: GetBucketWebsiteOutput) -> Self {
        Self {
            redirect_all_requests_to: value
                .redirect_all_requests_to
                .map(SerializableRedirectAllRequestsTo::from),
            index_document: value.index_document.map(SerializableIndexDocument::from),
            error_document: value.error_document.map(SerializableErrorDocument::from),
            routing_rules: value.routing_rules.map(|rules| {
                rules
                    .into_iter()
                    .map(SerializableRoutingRule::from)
                    .collect()
            }),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableGetBucketVersioningOutput {
    pub status: Option<SerializableBucketVersioningStatus>,
    pub mfa_delete: Option<SerializableMfaDeleteStatus>,
}
impl From<GetBucketVersioningOutput> for SerializableGetBucketVersioningOutput {
    fn from(value: GetBucketVersioningOutput) -> Self {
        Self {
            status: value.status.map(SerializableBucketVersioningStatus::from),
            mfa_delete: value.mfa_delete.map(SerializableMfaDeleteStatus::from),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum SerializableMfaDeleteStatus {
    Disabled,
    Enabled,
    Unknown,
}

impl From<MfaDeleteStatus> for SerializableMfaDeleteStatus {
    fn from(value: MfaDeleteStatus) -> Self {
        match value {
            MfaDeleteStatus::Disabled => Self::Disabled,
            MfaDeleteStatus::Enabled => Self::Enabled,
            _ => Self::Unknown,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableGetBucketTaggingOutput {
    pub tag_set: Vec<SerializableTag>,
}
impl From<GetBucketTaggingOutput> for SerializableGetBucketTaggingOutput {
    fn from(value: GetBucketTaggingOutput) -> Self {
        Self {
            tag_set: value
                .tag_set
                .into_iter()
                .map(SerializableTag::from)
                .collect(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableGetBucketRequestPaymentOutput {
    pub payer: Option<SerializablePayer>,
}
impl From<GetBucketRequestPaymentOutput> for SerializableGetBucketRequestPaymentOutput {
    fn from(value: GetBucketRequestPaymentOutput) -> Self {
        Self {
            payer: value.payer.map(SerializablePayer::from),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableGetBucketReplicationOutput {
    pub replication_configuration: Option<SerializableReplicationConfiguration>,
}
impl From<GetBucketReplicationOutput> for SerializableGetBucketReplicationOutput {
    fn from(value: GetBucketReplicationOutput) -> Self {
        Self {
            replication_configuration: value
                .replication_configuration
                .map(SerializableReplicationConfiguration::from),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]

pub struct SerializableGetBucketPolicyStatusOutput {
    pub policy_status: Option<SerializablePolicyStatus>,
}
impl From<GetBucketPolicyStatusOutput> for SerializableGetBucketPolicyStatusOutput {
    fn from(value: GetBucketPolicyStatusOutput) -> Self {
        Self {
            policy_status: value.policy_status.map(SerializablePolicyStatus::from),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializablePolicyStatus {
    pub is_public: Option<bool>,
}
impl From<PolicyStatus> for SerializablePolicyStatus {
    fn from(value: PolicyStatus) -> Self {
        Self {
            is_public: value.is_public,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableGetBucketPolicyOutput {
    pub policy: Option<String>,
}
impl From<GetBucketPolicyOutput> for SerializableGetBucketPolicyOutput {
    fn from(value: GetBucketPolicyOutput) -> Self {
        Self {
            policy: value.policy,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableGetBucketOwnershipControlsOutput {
    pub ownership_controls: Option<SerializableOwnershipControls>,
}
impl From<GetBucketOwnershipControlsOutput> for SerializableGetBucketOwnershipControlsOutput {
    fn from(value: GetBucketOwnershipControlsOutput) -> Self {
        Self {
            ownership_controls: value
                .ownership_controls
                .map(SerializableOwnershipControls::from),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableGetBucketNotificationConfigurationOutput {
    pub topic_configurations: Option<Vec<SerializableTopicConfiguration>>,
    pub queue_configurations: Option<Vec<SerializableQueueConfiguration>>,
    pub lambda_function_configurations: Option<Vec<SerializableLambdaFunctionConfiguration>>,
    pub event_bridge_configuration: Option<SerializableEventBridgeConfiguration>,
    /* private fields */
}
impl From<GetBucketNotificationConfigurationOutput>
    for SerializableGetBucketNotificationConfigurationOutput
{
    fn from(value: GetBucketNotificationConfigurationOutput) -> Self {
        Self {
            topic_configurations: value.topic_configurations.map(|v| {
                v.into_iter()
                    .map(SerializableTopicConfiguration::from)
                    .collect()
            }),
            queue_configurations: value.queue_configurations.map(|v| {
                v.into_iter()
                    .map(SerializableQueueConfiguration::from)
                    .collect()
            }),
            lambda_function_configurations: value.lambda_function_configurations.map(|v| {
                v.into_iter()
                    .map(SerializableLambdaFunctionConfiguration::from)
                    .collect()
            }),
            event_bridge_configuration: value
                .event_bridge_configuration
                .map(SerializableEventBridgeConfiguration::from),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableGetBucketMetricsConfigurationOutput {
    pub metrics_configuration: Option<SerializableMetricsConfiguration>,
}
impl From<GetBucketMetricsConfigurationOutput> for SerializableGetBucketMetricsConfigurationOutput {
    fn from(value: GetBucketMetricsConfigurationOutput) -> Self {
        Self {
            metrics_configuration: value
                .metrics_configuration
                .map(SerializableMetricsConfiguration::from),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableGetBucketLoggingOutput {
    pub logging_enabled: Option<SerializableLoggingEnabled>,
}
impl From<GetBucketLoggingOutput> for SerializableGetBucketLoggingOutput {
    fn from(value: GetBucketLoggingOutput) -> Self {
        Self {
            logging_enabled: value.logging_enabled.map(SerializableLoggingEnabled::from),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableGetBucketLocationOutput {
    pub location_constraint: Option<SerializableBucketLocationConstraint>,
}
impl From<GetBucketLocationOutput> for SerializableGetBucketLocationOutput {
    fn from(value: GetBucketLocationOutput) -> Self {
        Self {
            location_constraint: value
                .location_constraint
                .map(SerializableBucketLocationConstraint::from),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableGetBucketLifecycleConfigurationOutput {
    pub rules: Option<Vec<SerializableLifecycleRule>>,
}
impl From<GetBucketLifecycleConfigurationOutput>
    for SerializableGetBucketLifecycleConfigurationOutput
{
    fn from(value: GetBucketLifecycleConfigurationOutput) -> Self {
        Self {
            rules: value
                .rules
                .map(|v| v.into_iter().map(SerializableLifecycleRule::from).collect()),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableGetBucketInventoryConfigurationOutput {
    pub inventory_configuration: Option<SerializableInventoryConfiguration>,
    /* private fields */
}
impl From<GetBucketInventoryConfigurationOutput>
    for SerializableGetBucketInventoryConfigurationOutput
{
    fn from(value: GetBucketInventoryConfigurationOutput) -> Self {
        Self {
            inventory_configuration: value
                .inventory_configuration
                .map(SerializableInventoryConfiguration::from),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableGetBucketIntelligentTieringConfigurationOutput {
    pub intelligent_tiering_configuration: Option<SerializableIntelligentTieringConfiguration>,
}
impl From<GetBucketIntelligentTieringConfigurationOutput>
    for SerializableGetBucketIntelligentTieringConfigurationOutput
{
    fn from(value: GetBucketIntelligentTieringConfigurationOutput) -> Self {
        Self {
            intelligent_tiering_configuration: value
                .intelligent_tiering_configuration
                .map(SerializableIntelligentTieringConfiguration::from),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableGetBucketEncryptionOutput {
    pub server_side_encryption_configuration: Option<SerializableServerSideEncryptionConfiguration>,
}
impl From<GetBucketEncryptionOutput> for SerializableGetBucketEncryptionOutput {
    fn from(value: GetBucketEncryptionOutput) -> Self {
        Self {
            server_side_encryption_configuration: value
                .server_side_encryption_configuration
                .map(SerializableServerSideEncryptionConfiguration::from),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableGetBucketCorsOutput {
    pub cors_rules: Option<Vec<SerializableCorsRule>>,
}
impl From<GetBucketCorsOutput> for SerializableGetBucketCorsOutput {
    fn from(value: GetBucketCorsOutput) -> Self {
        Self {
            cors_rules: value
                .cors_rules
                .map(|v| v.into_iter().map(SerializableCorsRule::from).collect()),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableGetBucketAnalyticsConfigurationOutput {
    pub analytics_configuration: Option<SerializableAnalyticsConfiguration>,
}
impl From<GetBucketAnalyticsConfigurationOutput>
    for SerializableGetBucketAnalyticsConfigurationOutput
{
    fn from(value: GetBucketAnalyticsConfigurationOutput) -> Self {
        Self {
            analytics_configuration: value
                .analytics_configuration
                .map(SerializableAnalyticsConfiguration::from),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableGetBucketAclOutput {
    pub owner: Option<SerializableOwner>,
    pub grants: Option<Vec<SerializableGrant>>,
}
impl From<GetBucketAclOutput> for SerializableGetBucketAclOutput {
    fn from(value: GetBucketAclOutput) -> Self {
        Self {
            owner: value.owner.map(SerializableOwner::from),
            grants: value
                .grants
                .map(|v| v.into_iter().map(SerializableGrant::from).collect()),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableGetBucketAccelerateConfigurationOutput {
    pub status: Option<SerializableBucketAccelerateStatus>,
    pub request_charged: Option<SerializableRequestCharged>,
    /* private fields */
}
impl From<GetBucketAccelerateConfigurationOutput>
    for SerializableGetBucketAccelerateConfigurationOutput
{
    fn from(value: GetBucketAccelerateConfigurationOutput) -> Self {
        Self {
            status: value.status.map(SerializableBucketAccelerateStatus::from),
            request_charged: value.request_charged.map(SerializableRequestCharged::from),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum SerializableRequestCharged {
    Requester,
    Unknown,
}
impl From<RequestCharged> for SerializableRequestCharged {
    fn from(value: RequestCharged) -> Self {
        match value {
            RequestCharged::Requester => Self::Requester,
            _ => Self::Unknown,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct SerializableBucketInput {
    pub bucket: SerializableCreateBucketInput,
    pub accelerate_configuration: Option<SerializablePutBucketAccelerateConfigurationInput>,
    pub acl: Option<SerializablePutBucketAclInput>,
    pub analytics_configuration: Option<SerializablePutBucketAnalyticsConfigurationInput>,
    pub cors: Option<SerializablePutBucketCorsInput>,
    pub encryption: Option<SerializablePutBucketEncryptionInput>,
    pub intelligent_tiering_configuration:
        Option<SerializablePutBucketIntelligentTieringConfigurationInput>,
    pub inventory_configuration: Option<SerializablePutBucketInventoryConfigurationInput>,
    pub lifecycle_configuration: Option<SerializablePutBucketLifecycleConfigurationInput>,
    pub logging: Option<SerializablePutBucketLoggingInput>,
    pub metrics_configuration: Option<SerializablePutBucketMetricsConfigurationInput>,
    pub notification_configuration: Option<SerializablePutBucketNotificationConfigurationInput>,
    pub ownership_controls: Option<SerializablePutBucketOwnershipControlsInput>,
    pub policy: Option<SerializablePutBucketPolicyInput>,
    pub replication: Option<SerializablePutBucketReplicationInput>,
    pub request_payment: Option<SerializablePutBucketRequestPaymentInput>,
    pub tagging: Option<SerializablePutBucketTaggingInput>,
    pub versioning: Option<SerializablePutBucketVersioningInput>,
    pub website: Option<SerializablePutBucketWebsiteInput>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializablePutBucketWebsiteInput {
    pub bucket: Option<String>,
    pub content_md5: Option<String>,
    pub checksum_algorithm: Option<SerializableChecksumAlgorithm>,
    pub website_configuration: Option<SerializableWebsiteConfiguration>,
    pub expected_bucket_owner: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableWebsiteConfiguration {
    pub error_document: Option<SerializableErrorDocument>,
    pub index_document: Option<SerializableIndexDocument>,
    pub redirect_all_requests_to: Option<SerializableRedirectAllRequestsTo>,
    pub routing_rules: Option<Vec<SerializableRoutingRule>>,
}
impl From<WebsiteConfiguration> for SerializableWebsiteConfiguration {
    fn from(value: WebsiteConfiguration) -> Self {
        Self {
            error_document: value.error_document.map(|v| v.into()),
            index_document: value.index_document.map(|v| v.into()),
            redirect_all_requests_to: value.redirect_all_requests_to.map(|v| v.into()),
            routing_rules: value
                .routing_rules
                .map(|v| v.into_iter().map(|v| v.into()).collect()),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableRoutingRule {
    pub condition: Option<SerializableCondition>,
    pub redirect: Option<SerializableRedirect>,
}
impl From<RoutingRule> for SerializableRoutingRule {
    fn from(value: RoutingRule) -> Self {
        Self {
            condition: value.condition.map(|v| v.into()),
            redirect: value.redirect.map(|v| v.into()),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableRedirect {
    pub host_name: Option<String>,
    pub http_redirect_code: Option<String>,
    pub protocol: Option<SerializableProtocol>,
    pub replace_key_prefix_with: Option<String>,
    pub replace_key_with: Option<String>,
}
impl From<Redirect> for SerializableRedirect {
    fn from(value: Redirect) -> Self {
        Self {
            host_name: value.host_name,
            http_redirect_code: value.http_redirect_code,
            protocol: value.protocol.map(|v| v.into()),
            replace_key_prefix_with: value.replace_key_prefix_with,
            replace_key_with: value.replace_key_with,
        }
    }
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableCondition {
    pub http_error_code_returned_equals: Option<String>,
    pub key_prefix_equals: Option<String>,
}
impl From<Condition> for SerializableCondition {
    fn from(value: Condition) -> Self {
        Self {
            http_error_code_returned_equals: value.http_error_code_returned_equals,
            key_prefix_equals: value.key_prefix_equals,
        }
    }
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableRedirectAllRequestsTo {
    pub host_name: String,
    pub protocol: Option<SerializableProtocol>,
}
impl From<RedirectAllRequestsTo> for SerializableRedirectAllRequestsTo {
    fn from(value: RedirectAllRequestsTo) -> Self {
        Self {
            host_name: value.host_name,
            protocol: value.protocol.map(|v| v.into()),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]

pub enum SerializableProtocol {
    Http,
    Https,
    Unknown,
}
impl From<Protocol> for SerializableProtocol {
    fn from(value: Protocol) -> Self {
        match value {
            Protocol::Http => Self::Http,
            Protocol::Https => Self::Https,
            _ => Self::Unknown,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableIndexDocument {
    pub suffix: String,
}
impl From<IndexDocument> for SerializableIndexDocument {
    fn from(value: IndexDocument) -> Self {
        Self {
            suffix: value.suffix,
        }
    }
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableErrorDocument {
    pub key: String,
}
impl From<ErrorDocument> for SerializableErrorDocument {
    fn from(value: ErrorDocument) -> Self {
        Self { key: value.key }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializablePutBucketVersioningInput {
    pub bucket: Option<String>,
    pub content_md5: Option<String>,
    pub checksum_algorithm: Option<SerializableChecksumAlgorithm>,
    pub mfa: Option<String>,
    pub versioning_configuration: Option<SerializableVersioningConfiguration>,
    pub expected_bucket_owner: Option<String>,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableVersioningConfiguration {
    pub mfa_delete: Option<SerializableMfaDelete>,
    pub status: Option<SerializableBucketVersioningStatus>,
}
impl From<VersioningConfiguration> for SerializableVersioningConfiguration {
    fn from(value: VersioningConfiguration) -> Self {
        Self {
            mfa_delete: value.mfa_delete.map(|v| v.into()),
            status: value.status.map(|v| v.into()),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum SerializableBucketVersioningStatus {
    Enabled,
    Suspended,
    Unknown,
}
impl From<BucketVersioningStatus> for SerializableBucketVersioningStatus {
    fn from(value: BucketVersioningStatus) -> Self {
        match value {
            BucketVersioningStatus::Enabled => Self::Enabled,
            BucketVersioningStatus::Suspended => Self::Suspended,
            _ => Self::Unknown,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum SerializableMfaDelete {
    Disabled,
    Enabled,
    Unknown,
}
impl From<MfaDelete> for SerializableMfaDelete {
    fn from(value: MfaDelete) -> Self {
        match value {
            MfaDelete::Disabled => Self::Disabled,
            MfaDelete::Enabled => Self::Enabled,
            _ => Self::Unknown,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializablePutBucketTaggingInput {
    pub bucket: Option<String>,
    pub content_md5: Option<String>,
    pub checksum_algorithm: Option<SerializableChecksumAlgorithm>,
    pub tagging: Option<SerializableTagging>,
    pub expected_bucket_owner: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableTagging {
    pub tag_set: Vec<SerializableTag>,
}
impl From<Tagging> for SerializableTagging {
    fn from(value: Tagging) -> Self {
        Self {
            tag_set: value.tag_set.into_iter().map(|v| v.into()).collect(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializablePutBucketRequestPaymentInput {
    pub bucket: Option<String>,
    pub content_md5: Option<String>,
    pub checksum_algorithm: Option<SerializableChecksumAlgorithm>,
    pub request_payment_configuration: Option<SerializableRequestPaymentConfiguration>,
    pub expected_bucket_owner: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableRequestPaymentConfiguration {
    pub payer: SerializablePayer,
}
impl From<RequestPaymentConfiguration> for SerializableRequestPaymentConfiguration {
    fn from(value: RequestPaymentConfiguration) -> Self {
        Self {
            payer: value.payer.into(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum SerializablePayer {
    BucketOwner,
    Requester,
    Unknown,
}
impl From<Payer> for SerializablePayer {
    fn from(value: Payer) -> Self {
        match value {
            Payer::BucketOwner => Self::BucketOwner,
            Payer::Requester => Self::Requester,
            _ => Self::Unknown,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializablePutBucketReplicationInput {
    pub bucket: Option<String>,
    pub content_md5: Option<String>,
    pub checksum_algorithm: Option<SerializableChecksumAlgorithm>,
    pub replication_configuration: Option<SerializableReplicationConfiguration>,
    pub token: Option<String>,
    pub expected_bucket_owner: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableReplicationConfiguration {
    pub role: String,
    pub rules: Vec<SerializableReplicationRule>,
}
impl From<ReplicationConfiguration> for SerializableReplicationConfiguration {
    fn from(value: ReplicationConfiguration) -> Self {
        Self {
            role: value.role,
            rules: value.rules.into_iter().map(|v| v.into()).collect(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableReplicationRule {
    pub id: Option<String>,
    pub priority: Option<i32>,
    pub filter: Option<SerializableReplicationRuleFilter>,
    pub status: SerializableReplicationRuleStatus,
    pub source_selection_criteria: Option<SerializableSourceSelectionCriteria>,
    pub existing_object_replication: Option<SerializableExistingObjectReplication>,
    pub destination: Option<SerializableDestination>,
    pub delete_marker_replication: Option<SerializableDeleteMarkerReplication>,
}
impl From<ReplicationRule> for SerializableReplicationRule {
    fn from(value: ReplicationRule) -> Self {
        Self {
            id: value.id,
            priority: value.priority,
            filter: value.filter.map(|v| v.into()),
            status: value.status.into(),
            source_selection_criteria: value.source_selection_criteria.map(|v| v.into()),
            existing_object_replication: value.existing_object_replication.map(|v| v.into()),
            destination: value.destination.map(|v| v.into()),
            delete_marker_replication: value.delete_marker_replication.map(|v| v.into()),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableDeleteMarkerReplication {
    pub status: Option<SerializableDeleteMarkerReplicationStatus>,
}
impl From<DeleteMarkerReplication> for SerializableDeleteMarkerReplication {
    fn from(value: DeleteMarkerReplication) -> Self {
        Self {
            status: value.status.map(|v| v.into()),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum SerializableDeleteMarkerReplicationStatus {
    Disabled,
    Enabled,
    Unknown,
}

impl From<DeleteMarkerReplicationStatus> for SerializableDeleteMarkerReplicationStatus {
    fn from(value: DeleteMarkerReplicationStatus) -> Self {
        match value {
            DeleteMarkerReplicationStatus::Disabled => Self::Disabled,
            DeleteMarkerReplicationStatus::Enabled => Self::Enabled,
            _ => Self::Unknown,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableExistingObjectReplication {
    pub status: SerializableExistingObjectReplicationStatus,
}

impl From<ExistingObjectReplication> for SerializableExistingObjectReplication {
    fn from(value: ExistingObjectReplication) -> Self {
        Self {
            status: value.status.into(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum SerializableExistingObjectReplicationStatus {
    Disabled,
    Enabled,
    Unknown,
}
impl From<ExistingObjectReplicationStatus> for SerializableExistingObjectReplicationStatus {
    fn from(value: ExistingObjectReplicationStatus) -> Self {
        match value {
            ExistingObjectReplicationStatus::Disabled => Self::Disabled,
            ExistingObjectReplicationStatus::Enabled => Self::Enabled,
            _ => Self::Unknown,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableDestination {
    pub bucket: String,
    pub account: Option<String>,
    pub storage_class: Option<SerializableStorageClass>,
    pub access_control_translation: Option<SerializableAccessControlTranslation>,
    pub encryption_configuration: Option<SerializableEncryptionConfiguration>,
    pub replication_time: Option<SerializableReplicationTime>,
    pub metrics: Option<SerializableMetrics>,
}
impl From<Destination> for SerializableDestination {
    fn from(value: Destination) -> Self {
        Self {
            bucket: value.bucket,
            account: value.account,
            storage_class: value.storage_class.map(|v| v.into()),
            access_control_translation: value.access_control_translation.map(|v| v.into()),
            encryption_configuration: value.encryption_configuration.map(|v| v.into()),
            replication_time: value.replication_time.map(|v| v.into()),
            metrics: value.metrics.map(|v| v.into()),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableMetrics {
    pub status: SerializableMetricsStatus,
    pub event_threshold: Option<SerializableReplicationTimeValue>,
}
impl From<Metrics> for SerializableMetrics {
    fn from(value: Metrics) -> Self {
        Self {
            status: value.status.into(),
            event_threshold: value.event_threshold.map(|v| v.into()),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum SerializableMetricsStatus {
    Disabled,
    Enabled,
    Unknown,
}
impl From<MetricsStatus> for SerializableMetricsStatus {
    fn from(value: MetricsStatus) -> Self {
        match value {
            MetricsStatus::Disabled => Self::Disabled,
            MetricsStatus::Enabled => Self::Enabled,
            _ => Self::Unknown,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableReplicationTime {
    pub status: SerializableReplicationTimeStatus,
    pub time: Option<SerializableReplicationTimeValue>,
}
impl From<ReplicationTime> for SerializableReplicationTime {
    fn from(value: ReplicationTime) -> Self {
        Self {
            status: value.status.into(),
            time: value.time.map(|v| v.into()),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableReplicationTimeValue {
    pub minutes: Option<i32>,
}

impl From<ReplicationTimeValue> for SerializableReplicationTimeValue {
    fn from(value: ReplicationTimeValue) -> Self {
        Self {
            minutes: value.minutes,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum SerializableReplicationTimeStatus {
    Disabled,
    Enabled,
    Unknown,
}
impl From<ReplicationTimeStatus> for SerializableReplicationTimeStatus {
    fn from(value: ReplicationTimeStatus) -> Self {
        match value {
            ReplicationTimeStatus::Disabled => Self::Disabled,
            ReplicationTimeStatus::Enabled => Self::Enabled,
            _ => Self::Unknown,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableEncryptionConfiguration {
    pub replica_kms_key_id: Option<String>,
}
impl From<EncryptionConfiguration> for SerializableEncryptionConfiguration {
    fn from(value: EncryptionConfiguration) -> Self {
        Self {
            replica_kms_key_id: value.replica_kms_key_id,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableAccessControlTranslation {
    pub owner: SerializableOwnerOverride,
}
impl From<AccessControlTranslation> for SerializableAccessControlTranslation {
    fn from(value: AccessControlTranslation) -> Self {
        Self {
            owner: value.owner.into(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum SerializableOwnerOverride {
    Destination,
    Unknown,
}
impl From<OwnerOverride> for SerializableOwnerOverride {
    fn from(value: OwnerOverride) -> Self {
        match value {
            OwnerOverride::Destination => Self::Destination,
            _ => Self::Unknown,
        }
    }
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum SerializableStorageClass {
    DeepArchive,
    ExpressOnezone,
    Glacier,
    GlacierIr,
    IntelligentTiering,
    OnezoneIa,
    Outposts,
    ReducedRedundancy,
    Snow,
    Standard,
    StandardIa,
    Unknown,
}
impl From<StorageClass> for SerializableStorageClass {
    fn from(value: StorageClass) -> Self {
        match value {
            StorageClass::DeepArchive => Self::DeepArchive,
            StorageClass::ExpressOnezone => Self::ExpressOnezone,
            StorageClass::Glacier => Self::Glacier,
            StorageClass::GlacierIr => Self::GlacierIr,
            StorageClass::IntelligentTiering => Self::IntelligentTiering,
            StorageClass::OnezoneIa => Self::OnezoneIa,
            StorageClass::Outposts => Self::Outposts,
            StorageClass::ReducedRedundancy => Self::ReducedRedundancy,
            StorageClass::Snow => Self::Snow,
            StorageClass::Standard => Self::Standard,
            StorageClass::StandardIa => Self::StandardIa,
            _ => Self::Unknown,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableSourceSelectionCriteria {
    pub sse_kms_encrypted_objects: Option<SerializableSseKmsEncryptedObjects>,
    pub replica_modifications: Option<SerializableReplicaModifications>,
}
impl From<SourceSelectionCriteria> for SerializableSourceSelectionCriteria {
    fn from(value: SourceSelectionCriteria) -> Self {
        Self {
            sse_kms_encrypted_objects: value.sse_kms_encrypted_objects.map(|x| x.into()),
            replica_modifications: value.replica_modifications.map(|x| x.into()),
        }
    }
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableReplicaModifications {
    pub status: SerializableReplicaModificationsStatus,
}
impl From<ReplicaModifications> for SerializableReplicaModifications {
    fn from(value: ReplicaModifications) -> Self {
        Self {
            status: value.status.into(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum SerializableReplicaModificationsStatus {
    Disabled,
    Enabled,
    Unknown,
}
impl From<ReplicaModificationsStatus> for SerializableReplicaModificationsStatus {
    fn from(value: ReplicaModificationsStatus) -> Self {
        match value {
            ReplicaModificationsStatus::Disabled => Self::Disabled,
            ReplicaModificationsStatus::Enabled => Self::Enabled,
            _ => Self::Unknown,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableSseKmsEncryptedObjects {
    pub status: SerializableSseKmsEncryptedObjectsStatus,
}
impl From<SseKmsEncryptedObjects> for SerializableSseKmsEncryptedObjects {
    fn from(value: SseKmsEncryptedObjects) -> Self {
        Self {
            status: value.status.into(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]

pub enum SerializableSseKmsEncryptedObjectsStatus {
    Disabled,
    Enabled,
    Unknown,
}
impl From<SseKmsEncryptedObjectsStatus> for SerializableSseKmsEncryptedObjectsStatus {
    fn from(value: SseKmsEncryptedObjectsStatus) -> Self {
        match value {
            SseKmsEncryptedObjectsStatus::Disabled => Self::Disabled,
            SseKmsEncryptedObjectsStatus::Enabled => Self::Enabled,
            _ => Self::Unknown,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum SerializableReplicationRuleStatus {
    Disabled,
    Enabled,
    Unknown,
}
impl From<ReplicationRuleStatus> for SerializableReplicationRuleStatus {
    fn from(value: ReplicationRuleStatus) -> Self {
        match value {
            ReplicationRuleStatus::Disabled => Self::Disabled,
            ReplicationRuleStatus::Enabled => Self::Enabled,
            _ => Self::Unknown,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum SerializableReplicationRuleFilter {
    And(SerializableReplicationRuleAndOperator),
    Prefix(String),
    Tag(SerializableTag),
    Unknown,
}
impl From<ReplicationRuleFilter> for SerializableReplicationRuleFilter {
    fn from(value: ReplicationRuleFilter) -> Self {
        match value {
            ReplicationRuleFilter::And(and) => Self::And(and.into()),
            ReplicationRuleFilter::Prefix(prefix) => Self::Prefix(prefix),
            ReplicationRuleFilter::Tag(tag) => Self::Tag(tag.into()),
            _ => Self::Unknown,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableReplicationRuleAndOperator {
    pub prefix: Option<String>,
    pub tags: Option<Vec<SerializableTag>>,
}

impl From<ReplicationRuleAndOperator> for SerializableReplicationRuleAndOperator {
    fn from(value: ReplicationRuleAndOperator) -> Self {
        Self {
            prefix: value.prefix,
            tags: value
                .tags
                .map(|tags| tags.into_iter().map(|tag| tag.into()).collect()),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializablePutBucketPolicyInput {
    pub bucket: Option<String>,
    pub content_md5: Option<String>,
    pub checksum_algorithm: Option<SerializableChecksumAlgorithm>,
    pub confirm_remove_self_bucket_access: Option<bool>,
    pub policy: Option<String>,
    pub expected_bucket_owner: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializablePutBucketOwnershipControlsInput {
    pub bucket: Option<String>,
    pub content_md5: Option<String>,
    pub expected_bucket_owner: Option<String>,
    pub ownership_controls: Option<SerializableOwnershipControls>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableOwnershipControls {
    pub rules: Vec<SerializableOwnershipControlsRule>,
}
impl From<OwnershipControls> for SerializableOwnershipControls {
    fn from(value: OwnershipControls) -> Self {
        Self {
            rules: value.rules.into_iter().map(|rule| rule.into()).collect(),
        }
    }
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableOwnershipControlsRule {
    pub object_ownership: SerializableObjectOwnership,
}
impl From<OwnershipControlsRule> for SerializableOwnershipControlsRule {
    fn from(value: OwnershipControlsRule) -> Self {
        Self {
            object_ownership: value.object_ownership.into(),
        }
    }
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum SerializableObjectOwnership {
    BucketOwnerEnforced,
    BucketOwnerPreferred,
    ObjectWriter,
    Unknown,
}
impl From<ObjectOwnership> for SerializableObjectOwnership {
    fn from(value: ObjectOwnership) -> Self {
        match value {
            ObjectOwnership::BucketOwnerEnforced => Self::BucketOwnerEnforced,
            ObjectOwnership::BucketOwnerPreferred => Self::BucketOwnerPreferred,
            ObjectOwnership::ObjectWriter => Self::ObjectWriter,
            _ => Self::Unknown,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializablePutBucketNotificationConfigurationInput {
    pub bucket: Option<String>,
    pub notification_configuration: Option<SerializableNotificationConfiguration>,
    pub expected_bucket_owner: Option<String>,
    pub skip_destination_validation: Option<bool>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableNotificationConfiguration {
    pub topic_configurations: Option<Vec<SerializableTopicConfiguration>>,
    pub queue_configurations: Option<Vec<SerializableQueueConfiguration>>,
    pub lambda_function_configurations: Option<Vec<SerializableLambdaFunctionConfiguration>>,
    pub event_bridge_configuration: Option<SerializableEventBridgeConfiguration>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableEventBridgeConfiguration {}
impl From<EventBridgeConfiguration> for SerializableEventBridgeConfiguration {
    fn from(_value: EventBridgeConfiguration) -> Self {
        Self {}
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableLambdaFunctionConfiguration {
    pub id: Option<String>,
    pub lambda_function_arn: String,
    pub events: Vec<SerializableEvent>,
    pub filter: Option<SerializableNotificationConfigurationFilter>,
}
impl From<LambdaFunctionConfiguration> for SerializableLambdaFunctionConfiguration {
    fn from(value: LambdaFunctionConfiguration) -> Self {
        Self {
            id: value.id,
            lambda_function_arn: value.lambda_function_arn,
            events: value
                .events
                .into_iter()
                .map(SerializableEvent::from)
                .collect(),
            filter: value
                .filter
                .map(SerializableNotificationConfigurationFilter::from),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableQueueConfiguration {
    pub id: Option<String>,
    pub queue_arn: String,
    pub events: Vec<SerializableEvent>,
    pub filter: Option<SerializableNotificationConfigurationFilter>,
}
impl From<QueueConfiguration> for SerializableQueueConfiguration {
    fn from(value: QueueConfiguration) -> Self {
        Self {
            id: value.id,
            queue_arn: value.queue_arn,
            events: value
                .events
                .into_iter()
                .map(SerializableEvent::from)
                .collect(),
            filter: value
                .filter
                .map(SerializableNotificationConfigurationFilter::from),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableTopicConfiguration {
    pub id: Option<String>,
    pub topic_arn: String,
    pub events: Vec<SerializableEvent>,
    pub filter: Option<SerializableNotificationConfigurationFilter>,
}
impl From<TopicConfiguration> for SerializableTopicConfiguration {
    fn from(value: TopicConfiguration) -> Self {
        Self {
            id: value.id,
            topic_arn: value.topic_arn,
            events: value
                .events
                .into_iter()
                .map(SerializableEvent::from)
                .collect(),
            filter: value
                .filter
                .map(SerializableNotificationConfigurationFilter::from),
        }
    }
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableNotificationConfigurationFilter {
    pub key: Option<SerializableS3KeyFilter>,
}
impl From<NotificationConfigurationFilter> for SerializableNotificationConfigurationFilter {
    fn from(value: NotificationConfigurationFilter) -> Self {
        Self {
            key: value.key.map(SerializableS3KeyFilter::from),
        }
    }
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableS3KeyFilter {
    pub filter_rules: Option<Vec<SerializableFilterRule>>,
}
impl From<S3KeyFilter> for SerializableS3KeyFilter {
    fn from(value: S3KeyFilter) -> Self {
        Self {
            filter_rules: value.filter_rules.map(|rules| {
                rules
                    .into_iter()
                    .map(SerializableFilterRule::from)
                    .collect()
            }),
        }
    }
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableFilterRule {
    pub name: Option<SerializableFilterRuleName>,
    pub value: Option<String>,
}

impl From<FilterRule> for SerializableFilterRule {
    fn from(value: FilterRule) -> Self {
        Self {
            name: value.name.map(SerializableFilterRuleName::from),
            value: value.value,
        }
    }
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum SerializableFilterRuleName {
    Prefix,
    Suffix,
    Unknown,
}
impl From<FilterRuleName> for SerializableFilterRuleName {
    fn from(value: FilterRuleName) -> Self {
        match value {
            FilterRuleName::Prefix => Self::Prefix,
            FilterRuleName::Suffix => Self::Suffix,
            _ => Self::Unknown,
        }
    }
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum SerializableEvent {
    S3IntelligentTiering,
    S3LifecycleExpiration,
    S3LifecycleExpirationDelete,
    S3LifecycleExpirationDeleteMarkerCreated,
    S3LifecycleTransition,
    S3ObjectAclPut,
    S3ObjectCreated,
    S3ObjectCreatedCompleteMultipartUpload,
    S3ObjectCreatedCopy,
    S3ObjectCreatedPost,
    S3ObjectCreatedPut,
    S3ObjectRemoved,
    S3ObjectRemovedDelete,
    S3ObjectRemovedDeleteMarkerCreated,
    S3ObjectRestore,
    S3ObjectRestoreCompleted,
    S3ObjectRestoreDelete,
    S3ObjectRestorePost,
    S3ObjectTagging,
    S3ObjectTaggingDelete,
    S3ObjectTaggingPut,
    S3ReducedRedundancyLostObject,
    S3Replication,
    S3ReplicationOperationFailedReplication,
    S3ReplicationOperationMissedThreshold,
    S3ReplicationOperationNotTracked,
    S3ReplicationOperationReplicatedAfterThreshold,
    Unknown,
}
impl From<Event> for SerializableEvent {
    fn from(value: Event) -> Self {
        match value {
            Event::S3IntelligentTiering => Self::S3IntelligentTiering,
            Event::S3LifecycleExpiration => Self::S3LifecycleExpiration,
            Event::S3LifecycleExpirationDelete => Self::S3LifecycleExpirationDelete,
            Event::S3LifecycleExpirationDeleteMarkerCreated => {
                Self::S3LifecycleExpirationDeleteMarkerCreated
            }
            Event::S3LifecycleTransition => Self::S3LifecycleTransition,
            Event::S3ObjectAclPut => Self::S3ObjectAclPut,
            Event::S3ObjectCreated => Self::S3ObjectCreated,
            Event::S3ObjectCreatedCompleteMultipartUpload => {
                Self::S3ObjectCreatedCompleteMultipartUpload
            }
            Event::S3ObjectCreatedCopy => Self::S3ObjectCreatedCopy,
            Event::S3ObjectCreatedPost => Self::S3ObjectCreatedPost,
            Event::S3ObjectCreatedPut => Self::S3ObjectCreatedPut,
            Event::S3ObjectRemoved => Self::S3ObjectRemoved,
            Event::S3ObjectRemovedDelete => Self::S3ObjectRemovedDelete,
            Event::S3ObjectRemovedDeleteMarkerCreated => Self::S3ObjectRemovedDeleteMarkerCreated,
            Event::S3ObjectRestore => Self::S3ObjectRestore,
            Event::S3ObjectRestoreCompleted => Self::S3ObjectRestoreCompleted,
            Event::S3ObjectRestoreDelete => Self::S3ObjectRestoreDelete,
            Event::S3ObjectRestorePost => Self::S3ObjectRestorePost,
            Event::S3ObjectTagging => Self::S3ObjectTagging,
            Event::S3ObjectTaggingDelete => Self::S3ObjectTaggingDelete,
            Event::S3ObjectTaggingPut => Self::S3ObjectTaggingPut,
            Event::S3ReducedRedundancyLostObject => Self::S3ReducedRedundancyLostObject,
            Event::S3Replication => Self::S3Replication,
            Event::S3ReplicationOperationFailedReplication => {
                Self::S3ReplicationOperationFailedReplication
            }
            Event::S3ReplicationOperationMissedThreshold => {
                Self::S3ReplicationOperationMissedThreshold
            }
            Event::S3ReplicationOperationNotTracked => Self::S3ReplicationOperationNotTracked,
            Event::S3ReplicationOperationReplicatedAfterThreshold => {
                Self::S3ReplicationOperationReplicatedAfterThreshold
            }
            _ => Self::Unknown,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializablePutBucketMetricsConfigurationInput {
    pub bucket: Option<String>,
    pub id: Option<String>,
    pub metrics_configuration: Option<SerializableMetricsConfiguration>,
    pub expected_bucket_owner: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableMetricsConfiguration {
    pub id: String,
    pub filter: Option<SerializableMetricsFilter>,
}
impl From<MetricsConfiguration> for SerializableMetricsConfiguration {
    fn from(value: MetricsConfiguration) -> Self {
        Self {
            id: value.id,
            filter: value.filter.map(|x| x.into()),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum SerializableMetricsFilter {
    AccessPointArn(String),
    And(SerializableMetricsAndOperator),
    Prefix(String),
    Tag(SerializableTag),
    Unknown,
}
impl From<MetricsFilter> for SerializableMetricsFilter {
    fn from(value: MetricsFilter) -> Self {
        match value {
            MetricsFilter::AccessPointArn(x) => Self::AccessPointArn(x),
            MetricsFilter::And(x) => Self::And(x.into()),
            MetricsFilter::Prefix(x) => Self::Prefix(x),
            MetricsFilter::Tag(x) => Self::Tag(x.into()),
            _ => Self::Unknown,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableMetricsAndOperator {
    pub prefix: Option<String>,
    pub tags: Option<Vec<SerializableTag>>,
    pub access_point_arn: Option<String>,
}
impl From<MetricsAndOperator> for SerializableMetricsAndOperator {
    fn from(value: MetricsAndOperator) -> Self {
        Self {
            prefix: value.prefix,
            tags: value
                .tags
                .map(|x| x.into_iter().map(|x| x.into()).collect()),
            access_point_arn: value.access_point_arn,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializablePutBucketLoggingInput {
    pub bucket: Option<String>,
    pub bucket_logging_status: Option<SerializableBucketLoggingStatus>,
    pub content_md5: Option<String>,
    pub checksum_algorithm: Option<SerializableChecksumAlgorithm>,
    pub expected_bucket_owner: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableBucketLoggingStatus {
    pub logging_enabled: Option<SerializableLoggingEnabled>,
}
impl From<BucketLoggingStatus> for SerializableBucketLoggingStatus {
    fn from(value: BucketLoggingStatus) -> Self {
        Self {
            logging_enabled: value.logging_enabled.map(|x| x.into()),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableLoggingEnabled {
    pub target_bucket: String,
    pub target_grants: Option<Vec<SerializableTargetGrant>>,
    pub target_prefix: String,
    pub target_object_key_format: Option<SerializableTargetObjectKeyFormat>,
}
impl From<LoggingEnabled> for SerializableLoggingEnabled {
    fn from(value: LoggingEnabled) -> Self {
        Self {
            target_bucket: value.target_bucket,
            target_grants: value
                .target_grants
                .map(|x| x.into_iter().map(|x| x.into()).collect()),
            target_prefix: value.target_prefix,
            target_object_key_format: value.target_object_key_format.map(|x| x.into()),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]

pub struct SerializableTargetObjectKeyFormat {
    pub simple_prefix: Option<SerializableSimplePrefix>,
    pub partitioned_prefix: Option<SerializablePartitionedPrefix>,
}
impl From<TargetObjectKeyFormat> for SerializableTargetObjectKeyFormat {
    fn from(value: TargetObjectKeyFormat) -> Self {
        Self {
            simple_prefix: value.simple_prefix.map(|x| x.into()),
            partitioned_prefix: value.partitioned_prefix.map(|x| x.into()),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializablePartitionedPrefix {
    pub partition_date_source: Option<SerializablePartitionDateSource>,
}
impl From<PartitionedPrefix> for SerializablePartitionedPrefix {
    fn from(value: PartitionedPrefix) -> Self {
        Self {
            partition_date_source: value.partition_date_source.map(|x| x.into()),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum SerializablePartitionDateSource {
    DeliveryTime,
    EventTime,
    Unknown,
}
impl From<PartitionDateSource> for SerializablePartitionDateSource {
    fn from(value: PartitionDateSource) -> Self {
        match value {
            PartitionDateSource::DeliveryTime => Self::DeliveryTime,
            PartitionDateSource::EventTime => Self::EventTime,
            _ => Self::Unknown,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableSimplePrefix {}
impl From<SimplePrefix> for SerializableSimplePrefix {
    fn from(_: SimplePrefix) -> Self {
        Self {}
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableTargetGrant {
    pub grantee: Option<SerializableGrantee>,
    pub permission: Option<SerializableBucketLogsPermission>,
}
impl From<TargetGrant> for SerializableTargetGrant {
    fn from(value: TargetGrant) -> Self {
        Self {
            grantee: value.grantee.map(|x| x.into()),
            permission: value.permission.map(|x| x.into()),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum SerializableBucketLogsPermission {
    FullControl,
    Read,
    Write,
    Unknown,
}
impl From<BucketLogsPermission> for SerializableBucketLogsPermission {
    fn from(value: BucketLogsPermission) -> Self {
        match value {
            BucketLogsPermission::FullControl => Self::FullControl,
            BucketLogsPermission::Read => Self::Read,
            BucketLogsPermission::Write => Self::Write,
            _ => Self::Unknown,
        }
    }
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializablePutBucketLifecycleConfigurationInput {
    pub bucket: Option<String>,
    pub checksum_algorithm: Option<SerializableChecksumAlgorithm>,
    pub lifecycle_configuration: Option<SerializableBucketLifecycleConfiguration>,
    pub expected_bucket_owner: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableBucketLifecycleConfiguration {
    pub rules: Vec<SerializableLifecycleRule>,
}
impl From<BucketLifecycleConfiguration> for SerializableBucketLifecycleConfiguration {
    fn from(value: BucketLifecycleConfiguration) -> Self {
        Self {
            rules: value.rules.into_iter().map(|x| x.into()).collect(),
        }
    }
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableLifecycleRule {
    pub expiration: Option<SerializableLifecycleExpiration>,
    pub id: Option<String>,
    pub filter: Option<SerializableLifecycleRuleFilter>,
    pub status: SerializableExpirationStatus,
    pub transitions: Option<Vec<SerializableTransition>>,
    pub noncurrent_version_transitions: Option<Vec<SerializableNoncurrentVersionTransition>>,
    pub noncurrent_version_expiration: Option<SerializableNoncurrentVersionExpiration>,
    pub abort_incomplete_multipart_upload: Option<SerializableAbortIncompleteMultipartUpload>,
}
impl From<LifecycleRule> for SerializableLifecycleRule {
    fn from(value: LifecycleRule) -> Self {
        Self {
            expiration: value.expiration.map(|x| x.into()),
            id: value.id,
            filter: value.filter.map(|x| x.into()),
            status: value.status.into(),
            transitions: value
                .transitions
                .map(|v| v.into_iter().map(SerializableTransition::from).collect()),
            noncurrent_version_transitions: value.noncurrent_version_transitions.map(|v| {
                v.into_iter()
                    .map(SerializableNoncurrentVersionTransition::from)
                    .collect()
            }),
            noncurrent_version_expiration: value.noncurrent_version_expiration.map(|x| x.into()),
            abort_incomplete_multipart_upload: value
                .abort_incomplete_multipart_upload
                .map(|x| x.into()),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableAbortIncompleteMultipartUpload {
    pub days_after_initiation: Option<i32>,
}

impl From<AbortIncompleteMultipartUpload> for SerializableAbortIncompleteMultipartUpload {
    fn from(value: AbortIncompleteMultipartUpload) -> Self {
        Self {
            days_after_initiation: value.days_after_initiation,
        }
    }
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableNoncurrentVersionExpiration {
    pub noncurrent_days: Option<i32>,
    pub newer_noncurrent_versions: Option<i32>,
}
impl From<NoncurrentVersionExpiration> for SerializableNoncurrentVersionExpiration {
    fn from(value: NoncurrentVersionExpiration) -> Self {
        Self {
            noncurrent_days: value.noncurrent_days,
            newer_noncurrent_versions: value.newer_noncurrent_versions,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableNoncurrentVersionTransition {
    pub noncurrent_days: Option<i32>,
    pub storage_class: Option<SerializableTransitionStorageClass>,
    pub newer_noncurrent_versions: Option<i32>,
}
impl From<NoncurrentVersionTransition> for SerializableNoncurrentVersionTransition {
    fn from(value: NoncurrentVersionTransition) -> Self {
        Self {
            noncurrent_days: value.noncurrent_days,
            storage_class: value.storage_class.map(|x| x.into()),
            newer_noncurrent_versions: value.newer_noncurrent_versions,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableTransition {
    pub date: Option<u64>,
    pub days: Option<i32>,
    pub storage_class: Option<SerializableTransitionStorageClass>,
}
impl From<Transition> for SerializableTransition {
    fn from(value: Transition) -> Self {
        Self {
            date: value.date.map(|d| d.to_millis().unwrap_or_default() as u64),
            days: value.days,
            storage_class: value.storage_class.map(|x| x.into()),
        }
    }
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum SerializableTransitionStorageClass {
    DeepArchive,
    Glacier,
    GlacierIr,
    IntelligentTiering,
    OnezoneIa,
    StandardIa,
    Unknown,
}
impl From<TransitionStorageClass> for SerializableTransitionStorageClass {
    fn from(value: TransitionStorageClass) -> Self {
        match value {
            TransitionStorageClass::DeepArchive => Self::DeepArchive,
            TransitionStorageClass::Glacier => Self::Glacier,
            TransitionStorageClass::GlacierIr => Self::GlacierIr,
            TransitionStorageClass::IntelligentTiering => Self::IntelligentTiering,
            TransitionStorageClass::OnezoneIa => Self::OnezoneIa,
            TransitionStorageClass::StandardIa => Self::StandardIa,
            _ => Self::Unknown,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum SerializableExpirationStatus {
    Disabled,
    Enabled,
    Unknown,
}
impl From<ExpirationStatus> for SerializableExpirationStatus {
    fn from(value: ExpirationStatus) -> Self {
        match value {
            ExpirationStatus::Disabled => Self::Disabled,
            ExpirationStatus::Enabled => Self::Enabled,
            _ => Self::Unknown,
        }
    }
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum SerializableLifecycleRuleFilter {
    And(SerializableLifecycleRuleAndOperator),
    ObjectSizeGreaterThan(i64),
    ObjectSizeLessThan(i64),
    Prefix(String),
    Tag(SerializableTag),
    Unknown,
}
impl From<LifecycleRuleFilter> for SerializableLifecycleRuleFilter {
    fn from(value: LifecycleRuleFilter) -> Self {
        match value {
            LifecycleRuleFilter::And(x) => Self::And(x.into()),
            LifecycleRuleFilter::ObjectSizeGreaterThan(x) => Self::ObjectSizeGreaterThan(x),
            LifecycleRuleFilter::ObjectSizeLessThan(x) => Self::ObjectSizeLessThan(x),
            LifecycleRuleFilter::Prefix(x) => Self::Prefix(x),
            LifecycleRuleFilter::Tag(x) => Self::Tag(x.into()),
            _ => Self::Unknown,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableLifecycleRuleAndOperator {
    pub prefix: Option<String>,
    pub tags: Option<Vec<SerializableTag>>,
    pub object_size_greater_than: Option<i64>,
    pub object_size_less_than: Option<i64>,
}

impl From<LifecycleRuleAndOperator> for SerializableLifecycleRuleAndOperator {
    fn from(value: LifecycleRuleAndOperator) -> Self {
        Self {
            prefix: value.prefix,
            tags: value
                .tags
                .map(|x| x.into_iter().map(|x| x.into()).collect()),
            object_size_greater_than: value.object_size_greater_than,
            object_size_less_than: value.object_size_less_than,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableLifecycleExpiration {
    pub date: Option<u64>,
    pub days: Option<i32>,
    pub expired_object_delete_marker: Option<bool>,
}
impl From<LifecycleExpiration> for SerializableLifecycleExpiration {
    fn from(value: LifecycleExpiration) -> Self {
        Self {
            date: value.date.map(|d| d.to_millis().unwrap_or_default() as u64),
            days: value.days,
            expired_object_delete_marker: value.expired_object_delete_marker,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializablePutBucketInventoryConfigurationInput {
    pub bucket: Option<String>,
    pub id: Option<String>,
    pub inventory_configuration: Option<SerializableInventoryConfiguration>,
    pub expected_bucket_owner: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableInventoryConfiguration {
    pub destination: Option<SerializableInventoryDestination>,
    pub is_enabled: bool,
    pub filter: Option<SerializableInventoryFilter>,
    pub id: String,
    pub included_object_versions: SerializableInventoryIncludedObjectVersions,
    pub optional_fields: Option<Vec<SerializableInventoryOptionalField>>,
    pub schedule: Option<SerializableInventorySchedule>,
}
impl From<InventoryConfiguration> for SerializableInventoryConfiguration {
    fn from(value: InventoryConfiguration) -> Self {
        Self {
            destination: value.destination.map(|x| x.into()),
            is_enabled: value.is_enabled,
            filter: value.filter.map(|x| x.into()),
            id: value.id,
            included_object_versions: value.included_object_versions.into(),
            optional_fields: value
                .optional_fields
                .map(|x| x.into_iter().map(|x| x.into()).collect()),
            schedule: value.schedule.map(|x| x.into()),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableInventorySchedule {
    pub frequency: SerializableInventoryFrequency,
}
impl From<InventorySchedule> for SerializableInventorySchedule {
    fn from(value: InventorySchedule) -> Self {
        Self {
            frequency: value.frequency.into(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum SerializableInventoryFrequency {
    Daily,
    Weekly,
    Unknown,
}
impl From<InventoryFrequency> for SerializableInventoryFrequency {
    fn from(value: InventoryFrequency) -> Self {
        match value {
            InventoryFrequency::Daily => Self::Daily,
            InventoryFrequency::Weekly => Self::Weekly,
            _ => Self::Unknown,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum SerializableInventoryOptionalField {
    BucketKeyStatus,
    ChecksumAlgorithm,
    ETag,
    EncryptionStatus,
    IntelligentTieringAccessTier,
    IsMultipartUploaded,
    LastModifiedDate,
    ObjectAccessControlList,
    ObjectLockLegalHoldStatus,
    ObjectLockMode,
    ObjectLockRetainUntilDate,
    ObjectOwner,
    ReplicationStatus,
    Size,
    StorageClass,
    Unknown,
}
impl From<InventoryOptionalField> for SerializableInventoryOptionalField {
    fn from(value: InventoryOptionalField) -> Self {
        match value {
            InventoryOptionalField::BucketKeyStatus => Self::BucketKeyStatus,
            InventoryOptionalField::ChecksumAlgorithm => Self::ChecksumAlgorithm,
            InventoryOptionalField::ETag => Self::ETag,
            InventoryOptionalField::EncryptionStatus => Self::EncryptionStatus,
            InventoryOptionalField::IntelligentTieringAccessTier => {
                Self::IntelligentTieringAccessTier
            }
            InventoryOptionalField::IsMultipartUploaded => Self::IsMultipartUploaded,
            InventoryOptionalField::LastModifiedDate => Self::LastModifiedDate,
            InventoryOptionalField::ObjectAccessControlList => Self::ObjectAccessControlList,
            InventoryOptionalField::ObjectLockLegalHoldStatus => Self::ObjectLockLegalHoldStatus,
            InventoryOptionalField::ObjectLockMode => Self::ObjectLockMode,
            InventoryOptionalField::ObjectLockRetainUntilDate => Self::ObjectLockRetainUntilDate,
            InventoryOptionalField::ObjectOwner => Self::ObjectOwner,
            InventoryOptionalField::ReplicationStatus => Self::ReplicationStatus,
            InventoryOptionalField::Size => Self::Size,
            InventoryOptionalField::StorageClass => Self::StorageClass,
            _ => Self::Unknown,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum SerializableInventoryIncludedObjectVersions {
    All,
    Current,
    Unknown,
}
impl From<InventoryIncludedObjectVersions> for SerializableInventoryIncludedObjectVersions {
    fn from(value: InventoryIncludedObjectVersions) -> Self {
        match value {
            InventoryIncludedObjectVersions::All => Self::All,
            InventoryIncludedObjectVersions::Current => Self::Current,
            _ => Self::Unknown,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableInventoryFilter {
    pub prefix: String,
}
impl From<InventoryFilter> for SerializableInventoryFilter {
    fn from(value: InventoryFilter) -> Self {
        Self {
            prefix: value.prefix,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableInventoryDestination {
    pub s3_bucket_destination: Option<SerializableInventoryS3BucketDestination>,
}
impl From<InventoryDestination> for SerializableInventoryDestination {
    fn from(value: InventoryDestination) -> Self {
        Self {
            s3_bucket_destination: value
                .s3_bucket_destination
                .map(SerializableInventoryS3BucketDestination::from),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableInventoryS3BucketDestination {
    pub account_id: Option<String>,
    pub bucket: String,
    pub format: SerializableInventoryFormat,
    pub prefix: Option<String>,
    pub encryption: Option<SerializableInventoryEncryption>,
}
impl From<InventoryS3BucketDestination> for SerializableInventoryS3BucketDestination {
    fn from(value: InventoryS3BucketDestination) -> Self {
        Self {
            account_id: value.account_id,
            bucket: value.bucket,
            format: value.format.into(),
            prefix: value.prefix,
            encryption: value.encryption.map(SerializableInventoryEncryption::from),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableInventoryEncryption {
    pub sses3: Option<SerializableSses3>,
    pub ssekms: Option<SerializableSsekms>,
}
impl From<InventoryEncryption> for SerializableInventoryEncryption {
    fn from(value: InventoryEncryption) -> Self {
        Self {
            sses3: value.sses3.map(SerializableSses3::from),
            ssekms: value.ssekms.map(SerializableSsekms::from),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum SerializableInventoryFormat {
    Csv,
    Orc,
    Parquet,
    Unknown,
}
impl From<InventoryFormat> for SerializableInventoryFormat {
    fn from(value: InventoryFormat) -> Self {
        match value {
            InventoryFormat::Csv => Self::Csv,
            InventoryFormat::Orc => Self::Orc,
            InventoryFormat::Parquet => Self::Parquet,
            _ => Self::Unknown,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableSsekms {
    pub key_id: String,
}
impl From<Ssekms> for SerializableSsekms {
    fn from(a: Ssekms) -> Self {
        Self { key_id: a.key_id }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableSses3 {}
impl From<Sses3> for SerializableSses3 {
    fn from(_: Sses3) -> Self {
        Self {}
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializablePutBucketIntelligentTieringConfigurationInput {
    pub bucket: Option<String>,
    pub id: Option<String>,
    pub intelligent_tiering_configuration: Option<SerializableIntelligentTieringConfiguration>,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableIntelligentTieringConfiguration {
    pub id: String,
    pub filter: Option<SerializableIntelligentTieringFilter>,
    pub status: SerializableIntelligentTieringStatus,
    pub tierings: Vec<SerializableTiering>,
}
impl From<IntelligentTieringConfiguration> for SerializableIntelligentTieringConfiguration {
    fn from(value: IntelligentTieringConfiguration) -> Self {
        Self {
            id: value.id,
            filter: value.filter.map(SerializableIntelligentTieringFilter::from),
            status: value.status.into(),
            tierings: value
                .tierings
                .into_iter()
                .map(SerializableTiering::from)
                .collect(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableTiering {
    pub days: i32,
    pub access_tier: SerializableIntelligentTieringAccessTier,
}
impl From<Tiering> for SerializableTiering {
    fn from(value: Tiering) -> Self {
        Self {
            days: value.days,
            access_tier: value.access_tier.into(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum SerializableIntelligentTieringAccessTier {
    ArchiveAccess,
    DeepArchiveAccess,
    Unknown,
}

impl From<IntelligentTieringAccessTier> for SerializableIntelligentTieringAccessTier {
    fn from(value: IntelligentTieringAccessTier) -> Self {
        match value {
            IntelligentTieringAccessTier::ArchiveAccess => Self::ArchiveAccess,
            IntelligentTieringAccessTier::DeepArchiveAccess => Self::DeepArchiveAccess,
            _ => Self::Unknown,
        }
    }
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum SerializableIntelligentTieringStatus {
    Disabled,
    Enabled,
    Unknown,
}
impl From<IntelligentTieringStatus> for SerializableIntelligentTieringStatus {
    fn from(value: IntelligentTieringStatus) -> Self {
        match value {
            IntelligentTieringStatus::Disabled => Self::Disabled,
            IntelligentTieringStatus::Enabled => Self::Enabled,
            _ => Self::Unknown,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableIntelligentTieringFilter {
    pub prefix: Option<String>,
    pub tag: Option<SerializableTag>,
    pub and: Option<SerializableIntelligentTieringAndOperator>,
}
impl From<IntelligentTieringFilter> for SerializableIntelligentTieringFilter {
    fn from(value: IntelligentTieringFilter) -> Self {
        Self {
            prefix: value.prefix,
            tag: value.tag.map(SerializableTag::from),
            and: value
                .and
                .map(SerializableIntelligentTieringAndOperator::from),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableIntelligentTieringAndOperator {
    pub prefix: Option<String>,
    pub tags: Option<Vec<SerializableTag>>,
}
impl From<IntelligentTieringAndOperator> for SerializableIntelligentTieringAndOperator {
    fn from(value: IntelligentTieringAndOperator) -> Self {
        Self {
            prefix: value.prefix,
            tags: value
                .tags
                .map(|tags| tags.into_iter().map(SerializableTag::from).collect()),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializablePutBucketEncryptionInput {
    pub bucket: Option<String>,
    pub content_md5: Option<String>,
    pub checksum_algorithm: Option<SerializableChecksumAlgorithm>,
    pub server_side_encryption_configuration: Option<SerializableServerSideEncryptionConfiguration>,
    pub expected_bucket_owner: Option<String>,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableServerSideEncryptionConfiguration {
    pub rules: Vec<SerializableServerSideEncryptionRule>,
}
impl From<ServerSideEncryptionConfiguration> for SerializableServerSideEncryptionConfiguration {
    fn from(value: ServerSideEncryptionConfiguration) -> Self {
        Self {
            rules: value
                .rules
                .into_iter()
                .map(SerializableServerSideEncryptionRule::from)
                .collect(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableServerSideEncryptionRule {
    pub apply_server_side_encryption_by_default: Option<SerializableServerSideEncryptionByDefault>,
    pub bucket_key_enabled: Option<bool>,
}
impl From<ServerSideEncryptionRule> for SerializableServerSideEncryptionRule {
    fn from(value: ServerSideEncryptionRule) -> Self {
        Self {
            apply_server_side_encryption_by_default: value
                .apply_server_side_encryption_by_default
                .map(SerializableServerSideEncryptionByDefault::from),
            bucket_key_enabled: value.bucket_key_enabled,
        }
    }
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableServerSideEncryptionByDefault {
    pub sse_algorithm: SerializableServerSideEncryption,
    pub kms_master_key_id: Option<String>,
}
impl From<ServerSideEncryptionByDefault> for SerializableServerSideEncryptionByDefault {
    fn from(value: ServerSideEncryptionByDefault) -> Self {
        Self {
            sse_algorithm: SerializableServerSideEncryption::from(value.sse_algorithm),
            kms_master_key_id: value.kms_master_key_id,
        }
    }
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum SerializableServerSideEncryption {
    Aes256,
    AwsKms,
    AwsKmsDsse,
    Unknown,
}
impl From<ServerSideEncryption> for SerializableServerSideEncryption {
    fn from(value: ServerSideEncryption) -> Self {
        match value {
            ServerSideEncryption::Aes256 => Self::Aes256,
            ServerSideEncryption::AwsKms => Self::AwsKms,
            ServerSideEncryption::AwsKmsDsse => Self::AwsKmsDsse,
            _ => Self::Unknown,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializablePutBucketCorsInput {
    pub bucket: Option<String>,
    pub cors_configuration: Option<SerializableCorsConfiguration>,
    pub content_md5: Option<String>,
    pub checksum_algorithm: Option<SerializableChecksumAlgorithm>,
    pub expected_bucket_owner: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableCorsConfiguration {
    pub cors_rules: Vec<SerializableCorsRule>,
}
impl From<CorsConfiguration> for SerializableCorsConfiguration {
    fn from(value: CorsConfiguration) -> Self {
        Self {
            cors_rules: value
                .cors_rules
                .into_iter()
                .map(SerializableCorsRule::from)
                .collect(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableCorsRule {
    pub id: Option<String>,
    pub allowed_headers: Option<Vec<String>>,
    pub allowed_methods: Vec<String>,
    pub allowed_origins: Vec<String>,
    pub expose_headers: Option<Vec<String>>,
    pub max_age_seconds: Option<i32>,
}
impl From<CorsRule> for SerializableCorsRule {
    fn from(value: CorsRule) -> Self {
        Self {
            id: value.id,
            allowed_headers: value.allowed_headers,
            allowed_methods: value.allowed_methods,
            allowed_origins: value.allowed_origins,
            expose_headers: value.expose_headers,
            max_age_seconds: value.max_age_seconds,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializablePutBucketAnalyticsConfigurationInput {
    pub bucket: Option<String>,
    pub id: Option<String>,
    pub analytics_configuration: Option<SerializableAnalyticsConfiguration>,
    pub expected_bucket_owner: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableAnalyticsConfiguration {
    pub id: String,
    pub filter: Option<SerializableAnalyticsFilter>,
    pub storage_class_analysis: Option<SerializableStorageClassAnalysis>,
}
impl From<AnalyticsConfiguration> for SerializableAnalyticsConfiguration {
    fn from(value: AnalyticsConfiguration) -> Self {
        Self {
            id: value.id,
            filter: value.filter.map(SerializableAnalyticsFilter::from),
            storage_class_analysis: value
                .storage_class_analysis
                .map(SerializableStorageClassAnalysis::from),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableStorageClassAnalysis {
    pub data_export: Option<SerializableStorageClassAnalysisDataExport>,
}
impl From<StorageClassAnalysis> for SerializableStorageClassAnalysis {
    fn from(value: StorageClassAnalysis) -> Self {
        Self {
            data_export: value
                .data_export
                .map(SerializableStorageClassAnalysisDataExport::from),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableStorageClassAnalysisDataExport {
    pub output_schema_version: SerializableStorageClassAnalysisSchemaVersion,
    pub destination: Option<SerializableAnalyticsExportDestination>,
}
impl From<StorageClassAnalysisDataExport> for SerializableStorageClassAnalysisDataExport {
    fn from(value: StorageClassAnalysisDataExport) -> Self {
        Self {
            output_schema_version: SerializableStorageClassAnalysisSchemaVersion::from(
                value.output_schema_version,
            ),
            destination: value
                .destination
                .map(SerializableAnalyticsExportDestination::from),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableAnalyticsExportDestination {
    pub s3_bucket_destination: Option<SerializableAnalyticsS3BucketDestination>,
}
impl From<AnalyticsExportDestination> for SerializableAnalyticsExportDestination {
    fn from(value: AnalyticsExportDestination) -> Self {
        Self {
            s3_bucket_destination: value
                .s3_bucket_destination
                .map(SerializableAnalyticsS3BucketDestination::from),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableAnalyticsS3BucketDestination {
    pub format: SerializableAnalyticsS3ExportFileFormat,
    pub bucket_account_id: Option<String>,
    pub bucket: String,
    pub prefix: Option<String>,
}
impl From<AnalyticsS3BucketDestination> for SerializableAnalyticsS3BucketDestination {
    fn from(value: AnalyticsS3BucketDestination) -> Self {
        Self {
            format: SerializableAnalyticsS3ExportFileFormat::from(value.format),
            bucket_account_id: value.bucket_account_id,
            bucket: value.bucket,
            prefix: value.prefix,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum SerializableAnalyticsS3ExportFileFormat {
    Csv,
    Unknown,
}
impl From<AnalyticsS3ExportFileFormat> for SerializableAnalyticsS3ExportFileFormat {
    fn from(value: AnalyticsS3ExportFileFormat) -> Self {
        match value {
            AnalyticsS3ExportFileFormat::Csv => Self::Csv,
            _ => Self::Unknown,
        }
    }
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum SerializableStorageClassAnalysisSchemaVersion {
    V1,
    Unknown,
}
impl From<StorageClassAnalysisSchemaVersion> for SerializableStorageClassAnalysisSchemaVersion {
    fn from(value: StorageClassAnalysisSchemaVersion) -> Self {
        match value {
            StorageClassAnalysisSchemaVersion::V1 => Self::V1,
            _ => Self::Unknown,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum SerializableAnalyticsFilter {
    And(SerializableAnalyticsAndOperator),
    Prefix(String),
    Tag(SerializableTag),
    Unknown,
}
impl From<AnalyticsFilter> for SerializableAnalyticsFilter {
    fn from(value: AnalyticsFilter) -> Self {
        match value {
            AnalyticsFilter::And(v) => Self::And(SerializableAnalyticsAndOperator::from(v)),
            AnalyticsFilter::Prefix(v) => Self::Prefix(v),
            AnalyticsFilter::Tag(v) => Self::Tag(SerializableTag::from(v)),
            _ => Self::Unknown,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableAnalyticsAndOperator {
    pub prefix: Option<String>,
    pub tags: Option<Vec<SerializableTag>>,
}
impl From<AnalyticsAndOperator> for SerializableAnalyticsAndOperator {
    fn from(value: AnalyticsAndOperator) -> Self {
        Self {
            prefix: value.prefix,
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

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializablePutBucketAclInput {
    pub acl: Option<SerializableBucketCannedAcl>,
    pub access_control_policy: Option<SerializableAccessControlPolicy>,
    pub bucket: Option<String>,
    pub content_md5: Option<String>,
    pub checksum_algorithm: Option<SerializableChecksumAlgorithm>,
    pub grant_full_control: Option<String>,
    pub grant_read: Option<String>,
    pub grant_read_acp: Option<String>,
    pub grant_write: Option<String>,
    pub grant_write_acp: Option<String>,
    pub expected_bucket_owner: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableAccessControlPolicy {
    pub grants: Option<Vec<SerializableGrant>>,
    pub owner: Option<SerializableOwner>,
}
impl From<AccessControlPolicy> for SerializableAccessControlPolicy {
    fn from(value: AccessControlPolicy) -> Self {
        Self {
            grants: value
                .grants
                .map(|v| v.into_iter().map(SerializableGrant::from).collect()),
            owner: value.owner.map(SerializableOwner::from),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableOwner {
    pub display_name: Option<String>,
    pub id: Option<String>,
}
impl From<Owner> for SerializableOwner {
    fn from(value: Owner) -> Self {
        Self {
            display_name: value.display_name,
            id: value.id,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableGrant {
    pub grantee: Option<SerializableGrantee>,
    pub permission: Option<SerializablePermission>,
}
impl From<Grant> for SerializableGrant {
    fn from(value: Grant) -> Self {
        Self {
            grantee: value.grantee.map(SerializableGrantee::from),
            permission: value.permission.map(SerializablePermission::from),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum SerializablePermission {
    FullControl,
    Read,
    ReadAcp,
    Write,
    WriteAcp,
    Unknown,
}
impl From<Permission> for SerializablePermission {
    fn from(value: Permission) -> Self {
        match value {
            Permission::FullControl => Self::FullControl,
            Permission::Read => Self::Read,
            Permission::ReadAcp => Self::ReadAcp,
            Permission::Write => Self::Write,
            Permission::WriteAcp => Self::WriteAcp,
            _ => Self::Unknown,
        }
    }
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableGrantee {
    pub display_name: Option<String>,
    pub email_address: Option<String>,
    pub id: Option<String>,
    pub uri: Option<String>,
    pub r#type: SerializableType,
}
impl From<Grantee> for SerializableGrantee {
    fn from(value: Grantee) -> Self {
        Self {
            display_name: value.display_name,
            email_address: value.email_address,
            id: value.id,
            uri: value.uri,
            r#type: SerializableType::from(value.r#type),
        }
    }
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum SerializableType {
    AmazonCustomerByEmail,
    CanonicalUser,
    Group,
    Unknown,
}
impl From<Type> for SerializableType {
    fn from(value: Type) -> Self {
        match value {
            Type::AmazonCustomerByEmail => Self::AmazonCustomerByEmail,
            Type::CanonicalUser => Self::CanonicalUser,
            Type::Group => Self::Group,
            _ => Self::Unknown,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializablePutBucketAccelerateConfigurationInput {
    pub bucket: Option<String>,
    pub accelerate_configuration: Option<SerializableAccelerateConfiguration>,
    pub expected_bucket_owner: Option<String>,
    pub checksum_algorithm: Option<SerializableChecksumAlgorithm>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum SerializableChecksumAlgorithm {
    Crc32,
    Crc32C,
    Sha1,
    Sha256,
    Unknown,
}
impl From<ChecksumAlgorithm> for SerializableChecksumAlgorithm {
    fn from(value: ChecksumAlgorithm) -> Self {
        match value {
            ChecksumAlgorithm::Crc32 => Self::Crc32,
            ChecksumAlgorithm::Crc32C => Self::Crc32C,
            ChecksumAlgorithm::Sha1 => Self::Sha1,
            ChecksumAlgorithm::Sha256 => Self::Sha256,
            _ => Self::Unknown,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableAccelerateConfiguration {
    pub status: Option<SerializableBucketAccelerateStatus>,
}

impl From<AccelerateConfiguration> for SerializableAccelerateConfiguration {
    fn from(value: AccelerateConfiguration) -> Self {
        Self {
            status: value.status.map(SerializableBucketAccelerateStatus::from),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum SerializableBucketAccelerateStatus {
    Enabled,
    Suspended,
    Unknown,
}
impl From<BucketAccelerateStatus> for SerializableBucketAccelerateStatus {
    fn from(value: BucketAccelerateStatus) -> Self {
        match value {
            BucketAccelerateStatus::Enabled => Self::Enabled,
            BucketAccelerateStatus::Suspended => Self::Suspended,
            _ => Self::Unknown,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct SerializableCreateBucketInput {
    pub acl: Option<SerializableBucketCannedAcl>,
    pub bucket: Option<String>,
    pub create_bucket_configuration: Option<SerializableCreateBucketConfiguration>,
    pub grant_full_control: Option<String>,
    pub grant_read: Option<String>,
    pub grant_read_acp: Option<String>,
    pub grant_write: Option<String>,
    pub grant_write_acp: Option<String>,
    pub object_lock_enabled_for_bucket: Option<bool>,
    pub outpost_id: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum SerializableBucketCannedAcl {
    AuthenticatedRead,
    Private,
    PublicRead,
    PublicReadWrite,
    Unknown,
}
impl From<BucketCannedAcl> for SerializableBucketCannedAcl {
    fn from(value: BucketCannedAcl) -> Self {
        match value {
            BucketCannedAcl::AuthenticatedRead => Self::AuthenticatedRead,
            BucketCannedAcl::Private => Self::Private,
            BucketCannedAcl::PublicRead => Self::PublicRead,
            BucketCannedAcl::PublicReadWrite => Self::PublicReadWrite,
            _ => Self::Unknown,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct SerializableCreateBucketConfiguration {
    pub location_constraint: Option<SerializableBucketLocationConstraint>,
    pub location: Option<SerializableLocationInfo>,
    pub bucket: Option<SerializableBucketInfo>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum SerializableBucketType {
    Directory,
    Unknown,
}
impl From<BucketType> for SerializableBucketType {
    fn from(value: BucketType) -> Self {
        match value {
            BucketType::Directory => Self::Directory,
            _ => Self::Unknown,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableBucketInfo {
    pub data_redundancy: Option<SerializableDataRedundancy>,
    pub r#type: Option<SerializableBucketType>,
}
impl From<BucketInfo> for SerializableBucketInfo {
    fn from(value: BucketInfo) -> Self {
        Self {
            data_redundancy: value.data_redundancy.map(SerializableDataRedundancy::from),
            r#type: value.r#type.map(SerializableBucketType::from),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum SerializableDataRedundancy {
    SingleAvailabilityZone,
    Unknown,
}
impl From<DataRedundancy> for SerializableDataRedundancy {
    fn from(value: DataRedundancy) -> Self {
        match value {
            DataRedundancy::SingleAvailabilityZone => Self::SingleAvailabilityZone,
            _ => Self::Unknown,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableLocationInfo {
    pub r#type: Option<SerializableLocationType>,
    pub name: Option<String>,
}

impl From<LocationInfo> for SerializableLocationInfo {
    fn from(value: LocationInfo) -> Self {
        Self {
            r#type: value.r#type.map(SerializableLocationType::from),
            name: value.name,
        }
    }
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum SerializableLocationType {
    AvailabilityZone,
    Unknown,
}

impl From<LocationType> for SerializableLocationType {
    fn from(value: LocationType) -> Self {
        match value {
            LocationType::AvailabilityZone => Self::AvailabilityZone,
            _ => Self::Unknown,
        }
    }
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum SerializableBucketLocationConstraint {
    Eu,
    AfSouth1,
    ApEast1,
    ApNortheast1,
    ApNortheast2,
    ApNortheast3,
    ApSouth1,
    ApSouth2,
    ApSoutheast1,
    ApSoutheast2,
    ApSoutheast3,
    CaCentral1,
    CnNorth1,
    CnNorthwest1,
    EuCentral1,
    EuNorth1,
    EuSouth1,
    EuSouth2,
    EuWest1,
    EuWest2,
    EuWest3,
    MeSouth1,
    SaEast1,
    UsEast2,
    UsGovEast1,
    UsGovWest1,
    UsWest1,
    UsWest2,
    Unknown,
}

impl From<BucketLocationConstraint> for SerializableBucketLocationConstraint {
    fn from(value: BucketLocationConstraint) -> Self {
        match value {
            BucketLocationConstraint::Eu => Self::Eu,
            BucketLocationConstraint::AfSouth1 => Self::AfSouth1,
            BucketLocationConstraint::ApEast1 => Self::ApEast1,
            BucketLocationConstraint::ApNortheast1 => Self::ApNortheast1,
            BucketLocationConstraint::ApNortheast2 => Self::ApNortheast2,
            BucketLocationConstraint::ApNortheast3 => Self::ApNortheast3,
            BucketLocationConstraint::ApSouth1 => Self::ApSouth1,
            BucketLocationConstraint::ApSouth2 => Self::ApSouth2,
            BucketLocationConstraint::ApSoutheast1 => Self::ApSoutheast1,
            BucketLocationConstraint::ApSoutheast2 => Self::ApSoutheast2,
            BucketLocationConstraint::ApSoutheast3 => Self::ApSoutheast3,
            BucketLocationConstraint::CaCentral1 => Self::CaCentral1,
            BucketLocationConstraint::CnNorth1 => Self::CnNorth1,
            BucketLocationConstraint::CnNorthwest1 => Self::CnNorthwest1,
            BucketLocationConstraint::EuCentral1 => Self::EuCentral1,
            BucketLocationConstraint::EuNorth1 => Self::EuNorth1,
            BucketLocationConstraint::EuSouth1 => Self::EuSouth1,
            BucketLocationConstraint::EuSouth2 => Self::EuSouth2,
            BucketLocationConstraint::EuWest1 => Self::EuWest1,
            BucketLocationConstraint::EuWest2 => Self::EuWest2,
            BucketLocationConstraint::EuWest3 => Self::EuWest3,
            BucketLocationConstraint::MeSouth1 => Self::MeSouth1,
            BucketLocationConstraint::SaEast1 => Self::SaEast1,
            BucketLocationConstraint::UsEast2 => Self::UsEast2,
            BucketLocationConstraint::UsGovEast1 => Self::UsGovEast1,
            BucketLocationConstraint::UsGovWest1 => Self::UsGovWest1,
            BucketLocationConstraint::UsWest1 => Self::UsWest1,
            BucketLocationConstraint::UsWest2 => Self::UsWest2,
            _ => Self::Unknown,
        }
    }
}

impl From<SerializableRedirectAllRequestsTo> for RedirectAllRequestsTo {
    fn from(value: SerializableRedirectAllRequestsTo) -> Self {
        RedirectAllRequestsToBuilder::default()
            .host_name(value.host_name)
            .set_protocol(value.protocol.map(Protocol::from))
            .build()
            .unwrap()
    }
}

impl From<SerializableIndexDocument> for IndexDocument {
    fn from(value: SerializableIndexDocument) -> Self {
        IndexDocumentBuilder::default()
            .suffix(value.suffix)
            .build()
            .unwrap()
    }
}

impl From<SerializableErrorDocument> for ErrorDocument {
    fn from(value: SerializableErrorDocument) -> Self {
        ErrorDocumentBuilder::default()
            .key(value.key)
            .build()
            .unwrap()
    }
}

impl From<SerializableRoutingRule> for RoutingRule {
    fn from(value: SerializableRoutingRule) -> Self {
        RoutingRuleBuilder::default()
            .set_condition(value.condition.map(Condition::from))
            .set_redirect(value.redirect.map(Redirect::from))
            .build()
    }
}

impl From<SerializableCondition> for Condition {
    fn from(value: SerializableCondition) -> Self {
        ConditionBuilder::default()
            .set_http_error_code_returned_equals(value.http_error_code_returned_equals)
            .set_key_prefix_equals(value.key_prefix_equals)
            .build()
    }
}

impl From<SerializableRedirect> for Redirect {
    fn from(value: SerializableRedirect) -> Self {
        RedirectBuilder::default()
            .set_host_name(value.host_name)
            .set_http_redirect_code(value.http_redirect_code)
            .set_protocol(value.protocol.map(Protocol::from))
            .set_replace_key_prefix_with(value.replace_key_prefix_with)
            .set_replace_key_with(value.replace_key_with)
            .build()
    }
}

impl From<SerializableProtocol> for Protocol {
    fn from(value: SerializableProtocol) -> Self {
        match value {
            SerializableProtocol::Http => Self::Http,
            SerializableProtocol::Https => Self::Https,
            _ => Self::Https,
        }
    }
}

impl From<SerializableBucketVersioningStatus> for BucketVersioningStatus {
    fn from(value: SerializableBucketVersioningStatus) -> Self {
        match value {
            SerializableBucketVersioningStatus::Enabled => Self::Enabled,
            SerializableBucketVersioningStatus::Suspended => Self::Suspended,
            _ => Self::Enabled,
        }
    }
}

impl From<SerializableMfaDeleteStatus> for MfaDeleteStatus {
    fn from(value: SerializableMfaDeleteStatus) -> Self {
        match value {
            SerializableMfaDeleteStatus::Disabled => Self::Disabled,
            SerializableMfaDeleteStatus::Enabled => Self::Enabled,
            _ => Self::Disabled,
        }
    }
}

impl From<SerializableTag> for Tag {
    fn from(value: SerializableTag) -> Self {
        TagBuilder::default()
            .key(value.key)
            .value(value.value)
            .build()
            .unwrap()
    }
}

impl From<SerializablePayer> for Payer {
    fn from(value: SerializablePayer) -> Self {
        match value {
            SerializablePayer::BucketOwner => Self::BucketOwner,
            SerializablePayer::Requester => Self::Requester,
            _ => Self::BucketOwner,
        }
    }
}

impl From<SerializableReplicationConfiguration> for ReplicationConfiguration {
    fn from(value: SerializableReplicationConfiguration) -> Self {
        ReplicationConfigurationBuilder::default()
            .role(value.role.clone())
            .set_rules(Some(
                value.rules.into_iter().map(ReplicationRule::from).collect(),
            ))
            .build()
            .unwrap()
    }
}

impl From<SerializableReplicationRule> for ReplicationRule {
    fn from(value: SerializableReplicationRule) -> Self {
        ReplicationRuleBuilder::default()
            .set_id(value.id)
            .set_priority(value.priority)
            .set_filter(value.filter.map(ReplicationRuleFilter::from))
            .status(value.status.into())
            .set_source_selection_criteria(
                value
                    .source_selection_criteria
                    .map(SourceSelectionCriteria::from),
            )
            .set_existing_object_replication(
                value
                    .existing_object_replication
                    .map(ExistingObjectReplication::from),
            )
            .set_destination(value.destination.map(Destination::from))
            .set_delete_marker_replication(
                value
                    .delete_marker_replication
                    .map(DeleteMarkerReplication::from),
            )
            .build()
            .unwrap()
    }
}

impl From<SerializableDeleteMarkerReplication> for DeleteMarkerReplication {
    fn from(value: SerializableDeleteMarkerReplication) -> Self {
        DeleteMarkerReplicationBuilder::default()
            .set_status(value.status.map(DeleteMarkerReplicationStatus::from))
            .build()
    }
}

impl From<SerializableDeleteMarkerReplicationStatus> for DeleteMarkerReplicationStatus {
    fn from(value: SerializableDeleteMarkerReplicationStatus) -> Self {
        match value {
            SerializableDeleteMarkerReplicationStatus::Disabled => Self::Disabled,
            SerializableDeleteMarkerReplicationStatus::Enabled => Self::Enabled,
            _ => Self::Disabled,
        }
    }
}

impl From<SerializableExistingObjectReplication> for ExistingObjectReplication {
    fn from(value: SerializableExistingObjectReplication) -> Self {
        ExistingObjectReplicationBuilder::default()
            .status(value.status.clone().into())
            .build()
            .unwrap()
    }
}

impl From<SerializableExistingObjectReplicationStatus> for ExistingObjectReplicationStatus {
    fn from(value: SerializableExistingObjectReplicationStatus) -> Self {
        match value {
            SerializableExistingObjectReplicationStatus::Disabled => Self::Disabled,
            SerializableExistingObjectReplicationStatus::Enabled => Self::Enabled,
            _ => Self::Disabled,
        }
    }
}

impl From<SerializableDestination> for Destination {
    fn from(value: SerializableDestination) -> Self {
        DestinationBuilder::default()
            .bucket(value.bucket.clone())
            .set_account(value.account)
            .set_encryption_configuration(
                value
                    .encryption_configuration
                    .map(EncryptionConfiguration::from),
            )
            .set_metrics(value.metrics.map(Metrics::from))
            .set_replication_time(value.replication_time.map(ReplicationTime::from))
            .set_storage_class(value.storage_class.map(StorageClass::from))
            .build()
            .unwrap()
    }
}

impl From<SerializableMetrics> for Metrics {
    fn from(value: SerializableMetrics) -> Self {
        MetricsBuilder::default()
            .status(value.status.into())
            .set_event_threshold(value.event_threshold.map(ReplicationTimeValue::from))
            .build()
            .unwrap()
    }
}

impl From<SerializableMetricsStatus> for MetricsStatus {
    fn from(value: SerializableMetricsStatus) -> Self {
        match value {
            SerializableMetricsStatus::Disabled => Self::Disabled,
            SerializableMetricsStatus::Enabled => Self::Enabled,
            _ => Self::Disabled,
        }
    }
}

impl From<SerializableReplicationTime> for ReplicationTime {
    fn from(value: SerializableReplicationTime) -> Self {
        ReplicationTimeBuilder::default()
            .status(value.status.into())
            .set_time(value.time.map(ReplicationTimeValue::from))
            .build()
            .unwrap()
    }
}

impl From<SerializableReplicationTimeValue> for ReplicationTimeValue {
    fn from(value: SerializableReplicationTimeValue) -> Self {
        ReplicationTimeValueBuilder::default()
            .set_minutes(value.minutes)
            .build()
    }
}

impl From<SerializableReplicationTimeStatus> for ReplicationTimeStatus {
    fn from(value: SerializableReplicationTimeStatus) -> Self {
        match value {
            SerializableReplicationTimeStatus::Disabled => Self::Disabled,
            SerializableReplicationTimeStatus::Enabled => Self::Enabled,
            _ => Self::Disabled,
        }
    }
}

impl From<SerializableEncryptionConfiguration> for EncryptionConfiguration {
    fn from(value: SerializableEncryptionConfiguration) -> Self {
        EncryptionConfigurationBuilder::default()
            .set_replica_kms_key_id(value.replica_kms_key_id)
            .build()
    }
}

impl From<SerializableAccessControlTranslation> for AccessControlTranslation {
    fn from(value: SerializableAccessControlTranslation) -> Self {
        AccessControlTranslationBuilder::default()
            .owner(value.owner.into())
            .build()
            .unwrap()
    }
}

impl From<SerializableOwnerOverride> for OwnerOverride {
    fn from(value: SerializableOwnerOverride) -> Self {
        match value {
            SerializableOwnerOverride::Destination => Self::Destination,
            _ => Self::Destination,
        }
    }
}

impl From<SerializableStorageClass> for StorageClass {
    fn from(value: SerializableStorageClass) -> Self {
        match value {
            SerializableStorageClass::DeepArchive => Self::DeepArchive,
            SerializableStorageClass::ExpressOnezone => Self::ExpressOnezone,
            SerializableStorageClass::Glacier => Self::Glacier,
            SerializableStorageClass::GlacierIr => Self::GlacierIr,
            SerializableStorageClass::IntelligentTiering => Self::IntelligentTiering,
            SerializableStorageClass::OnezoneIa => Self::OnezoneIa,
            SerializableStorageClass::Outposts => Self::Outposts,
            SerializableStorageClass::ReducedRedundancy => Self::ReducedRedundancy,
            SerializableStorageClass::Snow => Self::Snow,
            SerializableStorageClass::Standard => Self::Standard,
            SerializableStorageClass::StandardIa => Self::StandardIa,
            _ => Self::Standard,
        }
    }
}

impl From<SerializableSourceSelectionCriteria> for SourceSelectionCriteria {
    fn from(value: SerializableSourceSelectionCriteria) -> Self {
        SourceSelectionCriteriaBuilder::default()
            .set_sse_kms_encrypted_objects(
                value
                    .sse_kms_encrypted_objects
                    .map(SseKmsEncryptedObjects::from),
            )
            .build()
    }
}

impl From<SerializableReplicaModifications> for ReplicaModifications {
    fn from(value: SerializableReplicaModifications) -> Self {
        ReplicaModificationsBuilder::default()
            .status(value.status.into())
            .build()
            .unwrap()
    }
}
impl From<SerializableReplicaModificationsStatus> for ReplicaModificationsStatus {
    fn from(value: SerializableReplicaModificationsStatus) -> Self {
        match value {
            SerializableReplicaModificationsStatus::Disabled => Self::Disabled,
            SerializableReplicaModificationsStatus::Enabled => Self::Enabled,
            _ => Self::Disabled,
        }
    }
}

impl From<SerializableSseKmsEncryptedObjects> for SseKmsEncryptedObjects {
    fn from(value: SerializableSseKmsEncryptedObjects) -> Self {
        SseKmsEncryptedObjectsBuilder::default()
            .status(value.status.into())
            .build()
            .unwrap()
    }
}

impl From<SerializableSseKmsEncryptedObjectsStatus> for SseKmsEncryptedObjectsStatus {
    fn from(value: SerializableSseKmsEncryptedObjectsStatus) -> Self {
        match value {
            SerializableSseKmsEncryptedObjectsStatus::Disabled => Self::Disabled,
            SerializableSseKmsEncryptedObjectsStatus::Enabled => Self::Enabled,
            _ => Self::Disabled,
        }
    }
}

impl From<SerializableReplicationRuleStatus> for ReplicationRuleStatus {
    fn from(value: SerializableReplicationRuleStatus) -> Self {
        match value {
            SerializableReplicationRuleStatus::Disabled => Self::Disabled,
            SerializableReplicationRuleStatus::Enabled => Self::Enabled,
            _ => Self::Disabled,
        }
    }
}

impl From<SerializableReplicationRuleFilter> for ReplicationRuleFilter {
    fn from(value: SerializableReplicationRuleFilter) -> Self {
        match value {
            SerializableReplicationRuleFilter::And(and) => Self::And(and.into()),
            SerializableReplicationRuleFilter::Prefix(prefix) => Self::Prefix(prefix),
            SerializableReplicationRuleFilter::Tag(tag) => Self::Tag(tag.into()),
            _ => Self::Unknown,
        }
    }
}

impl From<SerializableReplicationRuleAndOperator> for ReplicationRuleAndOperator {
    fn from(value: SerializableReplicationRuleAndOperator) -> Self {
        ReplicationRuleAndOperatorBuilder::default()
            .set_prefix(value.prefix)
            .set_tags(
                value
                    .tags
                    .map(|tags| tags.into_iter().map(Tag::from).collect()),
            )
            .build()
    }
}

impl From<SerializablePolicyStatus> for PolicyStatus {
    fn from(value: SerializablePolicyStatus) -> Self {
        PolicyStatusBuilder::default()
            .set_is_public(value.is_public)
            .build()
    }
}

impl From<SerializableOwnershipControls> for OwnershipControls {
    fn from(value: SerializableOwnershipControls) -> Self {
        OwnershipControlsBuilder::default()
            .set_rules(Some(
                value
                    .rules
                    .into_iter()
                    .map(OwnershipControlsRule::from)
                    .collect(),
            ))
            .build()
            .unwrap()
    }
}

impl From<SerializableOwnershipControlsRule> for OwnershipControlsRule {
    fn from(value: SerializableOwnershipControlsRule) -> Self {
        OwnershipControlsRuleBuilder::default()
            .object_ownership(value.object_ownership.into())
            .build()
            .unwrap()
    }
}

impl From<SerializableObjectOwnership> for ObjectOwnership {
    fn from(value: SerializableObjectOwnership) -> Self {
        match value {
            SerializableObjectOwnership::BucketOwnerEnforced => Self::BucketOwnerEnforced,
            SerializableObjectOwnership::BucketOwnerPreferred => Self::BucketOwnerPreferred,
            SerializableObjectOwnership::ObjectWriter => Self::ObjectWriter,
            _ => Self::BucketOwnerPreferred,
        }
    }
}

impl From<SerializableTopicConfiguration> for TopicConfiguration {
    fn from(value: SerializableTopicConfiguration) -> Self {
        TopicConfigurationBuilder::default()
            .set_id(value.id)
            .topic_arn(value.topic_arn)
            .set_events(Some(value.events.into_iter().map(Event::from).collect()))
            .set_filter(value.filter.map(NotificationConfigurationFilter::from))
            .build()
            .unwrap()
    }
}

impl From<SerializableQueueConfiguration> for QueueConfiguration {
    fn from(value: SerializableQueueConfiguration) -> Self {
        QueueConfigurationBuilder::default()
            .set_id(value.id)
            .queue_arn(value.queue_arn)
            .set_events(Some(value.events.into_iter().map(Event::from).collect()))
            .set_filter(value.filter.map(NotificationConfigurationFilter::from))
            .build()
            .unwrap()
    }
}

impl From<SerializableLambdaFunctionConfiguration> for LambdaFunctionConfiguration {
    fn from(value: SerializableLambdaFunctionConfiguration) -> Self {
        LambdaFunctionConfigurationBuilder::default()
            .set_id(value.id)
            .lambda_function_arn(value.lambda_function_arn)
            .set_events(Some(value.events.into_iter().map(Event::from).collect()))
            .set_filter(value.filter.map(NotificationConfigurationFilter::from))
            .build()
            .unwrap()
    }
}

impl From<SerializableEventBridgeConfiguration> for EventBridgeConfiguration {
    fn from(_: SerializableEventBridgeConfiguration) -> Self {
        EventBridgeConfigurationBuilder::default().build()
    }
}
impl From<SerializableNotificationConfiguration> for NotificationConfiguration {
    fn from(value: SerializableNotificationConfiguration) -> Self {
        NotificationConfigurationBuilder::default()
            .set_topic_configurations(
                value
                    .topic_configurations
                    .map(|configs| configs.into_iter().map(TopicConfiguration::from).collect()),
            )
            .set_queue_configurations(
                value
                    .queue_configurations
                    .map(|configs| configs.into_iter().map(QueueConfiguration::from).collect()),
            )
            .set_lambda_function_configurations(value.lambda_function_configurations.map(
                |configs| {
                    configs
                        .into_iter()
                        .map(LambdaFunctionConfiguration::from)
                        .collect()
                },
            ))
            .set_event_bridge_configuration(
                value
                    .event_bridge_configuration
                    .map(EventBridgeConfiguration::from),
            )
            .build()
    }
}

impl From<SerializableRequestPaymentConfiguration> for RequestPaymentConfiguration {
    fn from(value: SerializableRequestPaymentConfiguration) -> Self {
        RequestPaymentConfigurationBuilder::default()
            .payer(value.payer.into())
            .build()
            .unwrap()
    }
}
impl From<SerializableNotificationConfigurationFilter> for NotificationConfigurationFilter {
    fn from(value: SerializableNotificationConfigurationFilter) -> Self {
        NotificationConfigurationFilterBuilder::default()
            .set_key(value.key.map(S3KeyFilter::from))
            .build()
    }
}

impl From<SerializableS3KeyFilter> for S3KeyFilter {
    fn from(value: SerializableS3KeyFilter) -> Self {
        S3KeyFilterBuilder::default()
            .set_filter_rules(
                value
                    .filter_rules
                    .map(|rules| rules.into_iter().map(FilterRule::from).collect()),
            )
            .build()
    }
}

impl From<SerializableFilterRule> for FilterRule {
    fn from(value: SerializableFilterRule) -> Self {
        FilterRuleBuilder::default()
            .set_name(value.name.map(FilterRuleName::from))
            .set_value(value.value)
            .build()
    }
}

impl From<SerializableFilterRuleName> for FilterRuleName {
    fn from(value: SerializableFilterRuleName) -> Self {
        match value {
            SerializableFilterRuleName::Prefix => Self::Prefix,
            SerializableFilterRuleName::Suffix => Self::Suffix,
            _ => Self::Prefix,
        }
    }
}

impl From<SerializableEvent> for Event {
    fn from(value: SerializableEvent) -> Self {
        match value {
            SerializableEvent::S3IntelligentTiering => Self::S3IntelligentTiering,
            SerializableEvent::S3LifecycleExpiration => Self::S3LifecycleExpiration,
            SerializableEvent::S3LifecycleExpirationDelete => Self::S3LifecycleExpirationDelete,
            SerializableEvent::S3LifecycleExpirationDeleteMarkerCreated => {
                Self::S3LifecycleExpirationDeleteMarkerCreated
            }
            SerializableEvent::S3LifecycleTransition => Self::S3LifecycleTransition,
            SerializableEvent::S3ObjectAclPut => Self::S3ObjectAclPut,
            SerializableEvent::S3ObjectCreated => Self::S3ObjectCreated,
            SerializableEvent::S3ObjectCreatedCompleteMultipartUpload => {
                Self::S3ObjectCreatedCompleteMultipartUpload
            }
            SerializableEvent::S3ObjectCreatedCopy => Self::S3ObjectCreatedCopy,
            SerializableEvent::S3ObjectCreatedPost => Self::S3ObjectCreatedPost,
            SerializableEvent::S3ObjectCreatedPut => Self::S3ObjectCreatedPut,
            SerializableEvent::S3ObjectRemoved => Self::S3ObjectRemoved,
            SerializableEvent::S3ObjectRemovedDelete => Self::S3ObjectRemovedDelete,
            SerializableEvent::S3ObjectRemovedDeleteMarkerCreated => {
                Self::S3ObjectRemovedDeleteMarkerCreated
            }
            SerializableEvent::S3ObjectRestore => Self::S3ObjectRestore,
            SerializableEvent::S3ObjectRestoreCompleted => Self::S3ObjectRestoreCompleted,
            SerializableEvent::S3ObjectRestoreDelete => Self::S3ObjectRestoreDelete,
            SerializableEvent::S3ObjectRestorePost => Self::S3ObjectRestorePost,
            SerializableEvent::S3ObjectTagging => Self::S3ObjectTagging,
            SerializableEvent::S3ObjectTaggingDelete => Self::S3ObjectTaggingDelete,
            SerializableEvent::S3ObjectTaggingPut => Self::S3ObjectTaggingPut,
            SerializableEvent::S3ReducedRedundancyLostObject => Self::S3ReducedRedundancyLostObject,
            SerializableEvent::S3Replication => Self::S3Replication,
            SerializableEvent::S3ReplicationOperationFailedReplication => {
                Self::S3ReplicationOperationFailedReplication
            }
            SerializableEvent::S3ReplicationOperationMissedThreshold => {
                Self::S3ReplicationOperationMissedThreshold
            }
            SerializableEvent::S3ReplicationOperationNotTracked => {
                Self::S3ReplicationOperationNotTracked
            }
            SerializableEvent::S3ReplicationOperationReplicatedAfterThreshold => {
                Self::S3ReplicationOperationReplicatedAfterThreshold
            }
            SerializableEvent::Unknown => Self::S3IntelligentTiering,
        }
    }
}

impl From<SerializableMetricsConfiguration> for MetricsConfiguration {
    fn from(value: SerializableMetricsConfiguration) -> Self {
        MetricsConfigurationBuilder::default()
            .id(value.id)
            .set_filter(value.filter.map(MetricsFilter::from))
            .build()
            .unwrap()
    }
}

impl From<SerializableMetricsFilter> for MetricsFilter {
    fn from(value: SerializableMetricsFilter) -> Self {
        match value {
            SerializableMetricsFilter::AccessPointArn(v) => Self::AccessPointArn(v),
            SerializableMetricsFilter::And(v) => Self::And(MetricsAndOperator::from(v)),
            SerializableMetricsFilter::Prefix(v) => Self::Prefix(v),
            SerializableMetricsFilter::Tag(v) => Self::Tag(Tag::from(v)),
            SerializableMetricsFilter::Unknown => Self::Unknown,
        }
    }
}

impl From<SerializableMetricsAndOperator> for MetricsAndOperator {
    fn from(value: SerializableMetricsAndOperator) -> Self {
        MetricsAndOperatorBuilder::default()
            .set_prefix(value.prefix)
            .set_tags(
                value
                    .tags
                    .map(|tags| tags.into_iter().map(Tag::from).collect()),
            )
            .set_access_point_arn(value.access_point_arn)
            .build()
    }
}

impl From<SerializableBucketLoggingStatus> for BucketLoggingStatus {
    fn from(value: SerializableBucketLoggingStatus) -> Self {
        BucketLoggingStatusBuilder::default()
            .set_logging_enabled(value.logging_enabled.map(LoggingEnabled::from))
            .build()
    }
}

impl From<SerializableLoggingEnabled> for LoggingEnabled {
    fn from(value: SerializableLoggingEnabled) -> Self {
        LoggingEnabledBuilder::default()
            .target_bucket(value.target_bucket)
            .set_target_grants(
                value
                    .target_grants
                    .map(|grants| grants.into_iter().map(TargetGrant::from).collect()),
            )
            .target_prefix(value.target_prefix)
            .build()
            .unwrap()
    }
}

impl From<SerializableTargetObjectKeyFormat> for TargetObjectKeyFormat {
    fn from(value: SerializableTargetObjectKeyFormat) -> Self {
        TargetObjectKeyFormatBuilder::default()
            .set_simple_prefix(value.simple_prefix.map(SimplePrefix::from))
            .set_partitioned_prefix(value.partitioned_prefix.map(PartitionedPrefix::from))
            .build()
    }
}

impl From<SerializablePartitionedPrefix> for PartitionedPrefix {
    fn from(value: SerializablePartitionedPrefix) -> Self {
        PartitionedPrefixBuilder::default()
            .set_partition_date_source(value.partition_date_source.map(PartitionDateSource::from))
            .build()
    }
}

impl From<SerializablePartitionDateSource> for PartitionDateSource {
    fn from(value: SerializablePartitionDateSource) -> Self {
        match value {
            SerializablePartitionDateSource::DeliveryTime => Self::DeliveryTime,
            SerializablePartitionDateSource::EventTime => Self::EventTime,
            SerializablePartitionDateSource::Unknown => Self::DeliveryTime,
        }
    }
}

impl From<SerializableSimplePrefix> for SimplePrefix {
    fn from(_: SerializableSimplePrefix) -> Self {
        SimplePrefixBuilder::default().build()
    }
}

impl From<SerializableTargetGrant> for TargetGrant {
    fn from(value: SerializableTargetGrant) -> Self {
        TargetGrantBuilder::default()
            .set_grantee(value.grantee.map(Grantee::from))
            .set_permission(value.permission.map(BucketLogsPermission::from))
            .build()
    }
}

impl From<SerializableBucketLogsPermission> for BucketLogsPermission {
    fn from(value: SerializableBucketLogsPermission) -> Self {
        match value {
            SerializableBucketLogsPermission::FullControl => Self::FullControl,
            SerializableBucketLogsPermission::Read => Self::Read,
            SerializableBucketLogsPermission::Write => Self::Write,
            SerializableBucketLogsPermission::Unknown => Self::Read,
        }
    }
}

impl From<SerializableBucketLifecycleConfiguration> for BucketLifecycleConfiguration {
    fn from(value: SerializableBucketLifecycleConfiguration) -> Self {
        BucketLifecycleConfigurationBuilder::default()
            .set_rules(Some(
                value.rules.into_iter().map(LifecycleRule::from).collect(),
            ))
            .build()
            .unwrap()
    }
}

impl From<SerializableLifecycleRule> for LifecycleRule {
    fn from(value: SerializableLifecycleRule) -> Self {
        LifecycleRuleBuilder::default()
            .set_expiration(value.expiration.map(LifecycleExpiration::from))
            .set_id(value.id)
            .set_filter(value.filter.map(LifecycleRuleFilter::from))
            .set_status(Some(ExpirationStatus::from(value.status)))
            .set_transitions(
                value
                    .transitions
                    .map(|v| v.into_iter().map(Transition::from).collect()),
            )
            .set_noncurrent_version_transitions(value.noncurrent_version_transitions.map(|v| {
                v.into_iter()
                    .map(NoncurrentVersionTransition::from)
                    .collect()
            }))
            .set_noncurrent_version_expiration(
                value
                    .noncurrent_version_expiration
                    .map(NoncurrentVersionExpiration::from),
            )
            .set_abort_incomplete_multipart_upload(
                value
                    .abort_incomplete_multipart_upload
                    .map(AbortIncompleteMultipartUpload::from),
            )
            .build()
            .unwrap()
    }
}

impl From<SerializableAbortIncompleteMultipartUpload> for AbortIncompleteMultipartUpload {
    fn from(value: SerializableAbortIncompleteMultipartUpload) -> Self {
        AbortIncompleteMultipartUploadBuilder::default()
            .set_days_after_initiation(value.days_after_initiation)
            .build()
    }
}

impl From<SerializableNoncurrentVersionExpiration> for NoncurrentVersionExpiration {
    fn from(value: SerializableNoncurrentVersionExpiration) -> Self {
        NoncurrentVersionExpirationBuilder::default()
            .set_noncurrent_days(value.noncurrent_days)
            .build()
    }
}

impl From<SerializableNoncurrentVersionTransition> for NoncurrentVersionTransition {
    fn from(value: SerializableNoncurrentVersionTransition) -> Self {
        NoncurrentVersionTransitionBuilder::default()
            .set_noncurrent_days(value.noncurrent_days)
            .set_storage_class(value.storage_class.map(TransitionStorageClass::from))
            .build()
    }
}

impl From<SerializableTransition> for Transition {
    fn from(value: SerializableTransition) -> Self {
        TransitionBuilder::default()
            .set_date(value.date.map(|d| DateTime::from_millis(d as i64)))
            .set_days(value.days)
            .set_storage_class(value.storage_class.map(TransitionStorageClass::from))
            .build()
    }
}

impl From<SerializableTransitionStorageClass> for TransitionStorageClass {
    fn from(value: SerializableTransitionStorageClass) -> Self {
        match value {
            SerializableTransitionStorageClass::DeepArchive => Self::DeepArchive,
            SerializableTransitionStorageClass::Glacier => Self::Glacier,
            SerializableTransitionStorageClass::GlacierIr => Self::GlacierIr,
            SerializableTransitionStorageClass::IntelligentTiering => Self::IntelligentTiering,
            SerializableTransitionStorageClass::OnezoneIa => Self::OnezoneIa,
            SerializableTransitionStorageClass::StandardIa => Self::StandardIa,
            SerializableTransitionStorageClass::Unknown => Self::StandardIa,
        }
    }
}

impl From<SerializableExpirationStatus> for ExpirationStatus {
    fn from(value: SerializableExpirationStatus) -> Self {
        match value {
            SerializableExpirationStatus::Disabled => Self::Disabled,
            SerializableExpirationStatus::Enabled => Self::Enabled,
            SerializableExpirationStatus::Unknown => Self::Disabled,
        }
    }
}

impl From<SerializableLifecycleRuleFilter> for LifecycleRuleFilter {
    fn from(value: SerializableLifecycleRuleFilter) -> Self {
        match value {
            SerializableLifecycleRuleFilter::And(x) => Self::And(LifecycleRuleAndOperator::from(x)),
            SerializableLifecycleRuleFilter::ObjectSizeGreaterThan(x) => {
                Self::ObjectSizeGreaterThan(x)
            }
            SerializableLifecycleRuleFilter::ObjectSizeLessThan(x) => Self::ObjectSizeLessThan(x),
            SerializableLifecycleRuleFilter::Prefix(x) => Self::Prefix(x),
            SerializableLifecycleRuleFilter::Tag(x) => Self::Tag(Tag::from(x)),
            SerializableLifecycleRuleFilter::Unknown => Self::Unknown,
        }
    }
}

impl From<SerializableLifecycleRuleAndOperator> for LifecycleRuleAndOperator {
    fn from(value: SerializableLifecycleRuleAndOperator) -> Self {
        LifecycleRuleAndOperatorBuilder::default()
            .set_prefix(value.prefix)
            .set_tags(value.tags.map(|x| x.into_iter().map(Tag::from).collect()))
            .build()
    }
}

impl From<SerializableLifecycleExpiration> for LifecycleExpiration {
    fn from(value: SerializableLifecycleExpiration) -> Self {
        LifecycleExpirationBuilder::default()
            .set_date(value.date.map(|d| DateTime::from_millis(d as i64)))
            .set_days(value.days)
            .build()
    }
}

impl From<SerializableInventoryConfiguration> for InventoryConfiguration {
    fn from(value: SerializableInventoryConfiguration) -> Self {
        InventoryConfigurationBuilder::default()
            .set_destination(value.destination.map(InventoryDestination::from))
            .set_is_enabled(Some(value.is_enabled))
            .set_filter(value.filter.map(InventoryFilter::from))
            .set_id(Some(value.id))
            .set_included_object_versions(Some(value.included_object_versions.into()))
            .set_optional_fields(
                value
                    .optional_fields
                    .map(|x| x.into_iter().map(InventoryOptionalField::from).collect()),
            )
            .set_schedule(value.schedule.map(InventorySchedule::from))
            .build()
            .unwrap()
    }
}

impl From<SerializableInventorySchedule> for InventorySchedule {
    fn from(value: SerializableInventorySchedule) -> Self {
        InventoryScheduleBuilder::default()
            .set_frequency(Some(value.frequency.into()))
            .build()
            .unwrap()
    }
}

impl From<SerializableInventoryFrequency> for InventoryFrequency {
    fn from(value: SerializableInventoryFrequency) -> Self {
        match value {
            SerializableInventoryFrequency::Daily => Self::Daily,
            SerializableInventoryFrequency::Weekly => Self::Weekly,
            SerializableInventoryFrequency::Unknown => Self::Daily,
        }
    }
}

impl From<SerializableInventoryOptionalField> for InventoryOptionalField {
    fn from(value: SerializableInventoryOptionalField) -> Self {
        match value {
            SerializableInventoryOptionalField::BucketKeyStatus => Self::BucketKeyStatus,
            SerializableInventoryOptionalField::ChecksumAlgorithm => Self::ChecksumAlgorithm,
            SerializableInventoryOptionalField::ETag => Self::ETag,
            SerializableInventoryOptionalField::EncryptionStatus => Self::EncryptionStatus,
            SerializableInventoryOptionalField::IntelligentTieringAccessTier => {
                Self::IntelligentTieringAccessTier
            }
            SerializableInventoryOptionalField::IsMultipartUploaded => Self::IsMultipartUploaded,
            SerializableInventoryOptionalField::LastModifiedDate => Self::LastModifiedDate,
            SerializableInventoryOptionalField::ObjectAccessControlList => {
                Self::ObjectAccessControlList
            }
            SerializableInventoryOptionalField::ObjectLockLegalHoldStatus => {
                Self::ObjectLockLegalHoldStatus
            }
            SerializableInventoryOptionalField::ObjectLockMode => Self::ObjectLockMode,
            SerializableInventoryOptionalField::ObjectLockRetainUntilDate => {
                Self::ObjectLockRetainUntilDate
            }
            SerializableInventoryOptionalField::ObjectOwner => Self::ObjectOwner,
            SerializableInventoryOptionalField::ReplicationStatus => Self::ReplicationStatus,
            SerializableInventoryOptionalField::Size => Self::Size,
            SerializableInventoryOptionalField::StorageClass => Self::StorageClass,
            SerializableInventoryOptionalField::Unknown => Self::Size,
        }
    }
}

impl From<SerializableInventoryIncludedObjectVersions> for InventoryIncludedObjectVersions {
    fn from(value: SerializableInventoryIncludedObjectVersions) -> Self {
        match value {
            SerializableInventoryIncludedObjectVersions::All => Self::All,
            SerializableInventoryIncludedObjectVersions::Current => Self::Current,
            SerializableInventoryIncludedObjectVersions::Unknown => Self::Current,
        }
    }
}

impl From<SerializableInventoryFilter> for InventoryFilter {
    fn from(value: SerializableInventoryFilter) -> Self {
        InventoryFilterBuilder::default()
            .set_prefix(Some(value.prefix))
            .build()
            .unwrap()
    }
}

impl From<SerializableInventoryDestination> for InventoryDestination {
    fn from(value: SerializableInventoryDestination) -> Self {
        InventoryDestinationBuilder::default()
            .set_s3_bucket_destination(
                value
                    .s3_bucket_destination
                    .map(InventoryS3BucketDestination::from),
            )
            .build()
    }
}

impl From<SerializableInventoryS3BucketDestination> for InventoryS3BucketDestination {
    fn from(value: SerializableInventoryS3BucketDestination) -> Self {
        InventoryS3BucketDestinationBuilder::default()
            .set_account_id(value.account_id)
            .set_bucket(Some(value.bucket))
            .set_format(Some(value.format.into()))
            .set_prefix(value.prefix)
            .set_encryption(value.encryption.map(InventoryEncryption::from))
            .build()
            .unwrap()
    }
}

impl From<SerializableInventoryEncryption> for InventoryEncryption {
    fn from(value: SerializableInventoryEncryption) -> Self {
        InventoryEncryptionBuilder::default()
            .set_ssekms(value.ssekms.map(Ssekms::from))
            .set_sses3(value.sses3.map(Sses3::from))
            .build()
    }
}

impl From<SerializableInventoryFormat> for InventoryFormat {
    fn from(value: SerializableInventoryFormat) -> Self {
        match value {
            SerializableInventoryFormat::Csv => Self::Csv,
            SerializableInventoryFormat::Orc => Self::Orc,
            SerializableInventoryFormat::Parquet => Self::Parquet,
            SerializableInventoryFormat::Unknown => Self::Csv,
        }
    }
}

impl From<SerializableSsekms> for Ssekms {
    fn from(a: SerializableSsekms) -> Self {
        SsekmsBuilder::default()
            .set_key_id(Some(a.key_id))
            .build()
            .unwrap()
    }
}

impl From<SerializableSses3> for Sses3 {
    fn from(_: SerializableSses3) -> Self {
        Sses3Builder::default().build()
    }
}

impl From<SerializableIntelligentTieringConfiguration> for IntelligentTieringConfiguration {
    fn from(value: SerializableIntelligentTieringConfiguration) -> Self {
        IntelligentTieringConfigurationBuilder::default()
            .set_id(Some(value.id))
            .set_filter(value.filter.map(IntelligentTieringFilter::from))
            .set_status(Some(value.status.into()))
            .set_tierings(Some(
                value.tierings.into_iter().map(Tiering::from).collect(),
            ))
            .build()
            .unwrap()
    }
}

impl From<SerializableTiering> for Tiering {
    fn from(value: SerializableTiering) -> Self {
        TieringBuilder::default()
            .set_access_tier(Some(value.access_tier.into()))
            .set_days(Some(value.days))
            .build()
            .unwrap()
    }
}

impl From<SerializableIntelligentTieringAccessTier> for IntelligentTieringAccessTier {
    fn from(value: SerializableIntelligentTieringAccessTier) -> Self {
        match value {
            SerializableIntelligentTieringAccessTier::ArchiveAccess => Self::ArchiveAccess,
            SerializableIntelligentTieringAccessTier::DeepArchiveAccess => Self::DeepArchiveAccess,
            SerializableIntelligentTieringAccessTier::Unknown => Self::ArchiveAccess,
        }
    }
}

impl From<SerializableIntelligentTieringStatus> for IntelligentTieringStatus {
    fn from(value: SerializableIntelligentTieringStatus) -> Self {
        match value {
            SerializableIntelligentTieringStatus::Disabled => Self::Disabled,
            SerializableIntelligentTieringStatus::Enabled => Self::Enabled,
            SerializableIntelligentTieringStatus::Unknown => Self::Disabled,
        }
    }
}

impl From<SerializableIntelligentTieringFilter> for IntelligentTieringFilter {
    fn from(value: SerializableIntelligentTieringFilter) -> Self {
        IntelligentTieringFilterBuilder::default()
            .set_prefix(value.prefix)
            .build()
    }
}

impl From<SerializableIntelligentTieringAndOperator> for IntelligentTieringAndOperator {
    fn from(value: SerializableIntelligentTieringAndOperator) -> Self {
        IntelligentTieringAndOperatorBuilder::default()
            .set_prefix(value.prefix)
            .set_tags(value.tags.map(|x| x.into_iter().map(Tag::from).collect()))
            .build()
    }
}

impl From<SerializableServerSideEncryptionConfiguration> for ServerSideEncryptionConfiguration {
    fn from(value: SerializableServerSideEncryptionConfiguration) -> Self {
        ServerSideEncryptionConfigurationBuilder::default()
            .set_rules(Some(
                value
                    .rules
                    .into_iter()
                    .map(ServerSideEncryptionRule::from)
                    .collect(),
            ))
            .build()
            .unwrap()
    }
}

impl From<SerializableServerSideEncryptionRule> for ServerSideEncryptionRule {
    fn from(value: SerializableServerSideEncryptionRule) -> Self {
        ServerSideEncryptionRuleBuilder::default()
            .set_apply_server_side_encryption_by_default(
                value
                    .apply_server_side_encryption_by_default
                    .map(ServerSideEncryptionByDefault::from),
            )
            .build()
    }
}

impl From<SerializableServerSideEncryptionByDefault> for ServerSideEncryptionByDefault {
    fn from(value: SerializableServerSideEncryptionByDefault) -> Self {
        ServerSideEncryptionByDefaultBuilder::default()
            .set_sse_algorithm(Some(value.sse_algorithm.into()))
            .set_kms_master_key_id(value.kms_master_key_id)
            .build()
            .unwrap()
    }
}

impl From<SerializableServerSideEncryption> for ServerSideEncryption {
    fn from(value: SerializableServerSideEncryption) -> Self {
        match value {
            SerializableServerSideEncryption::Aes256 => Self::Aes256,
            SerializableServerSideEncryption::AwsKms => Self::AwsKms,
            SerializableServerSideEncryption::AwsKmsDsse => Self::AwsKmsDsse,
            SerializableServerSideEncryption::Unknown => Self::Aes256,
        }
    }
}

impl From<SerializableCorsConfiguration> for CorsConfiguration {
    fn from(value: SerializableCorsConfiguration) -> Self {
        CorsConfigurationBuilder::default()
            .set_cors_rules(Some(
                value.cors_rules.into_iter().map(CorsRule::from).collect(),
            ))
            .build()
            .unwrap()
    }
}

impl From<SerializableCorsRule> for CorsRule {
    fn from(value: SerializableCorsRule) -> Self {
        CorsRuleBuilder::default()
            .set_allowed_headers(value.allowed_headers)
            .set_allowed_methods(Some(value.allowed_methods))
            .set_allowed_origins(Some(value.allowed_origins))
            .set_expose_headers(value.expose_headers)
            .set_max_age_seconds(value.max_age_seconds)
            .build()
            .unwrap()
    }
}

impl From<SerializableAnalyticsConfiguration> for AnalyticsConfiguration {
    fn from(value: SerializableAnalyticsConfiguration) -> Self {
        AnalyticsConfigurationBuilder::default()
            .set_id(Some(value.id))
            .set_filter(value.filter.map(AnalyticsFilter::from))
            .set_storage_class_analysis(
                value.storage_class_analysis.map(StorageClassAnalysis::from),
            )
            .build()
            .unwrap()
    }
}

impl From<SerializableStorageClassAnalysis> for StorageClassAnalysis {
    fn from(value: SerializableStorageClassAnalysis) -> Self {
        StorageClassAnalysisBuilder::default()
            .set_data_export(value.data_export.map(StorageClassAnalysisDataExport::from))
            .build()
    }
}

impl From<SerializableStorageClassAnalysisDataExport> for StorageClassAnalysisDataExport {
    fn from(value: SerializableStorageClassAnalysisDataExport) -> Self {
        StorageClassAnalysisDataExportBuilder::default()
            .set_output_schema_version(Some(value.output_schema_version.into()))
            .set_destination(value.destination.map(AnalyticsExportDestination::from))
            .build()
            .unwrap()
    }
}

impl From<SerializableAnalyticsExportDestination> for AnalyticsExportDestination {
    fn from(value: SerializableAnalyticsExportDestination) -> Self {
        AnalyticsExportDestinationBuilder::default()
            .set_s3_bucket_destination(
                value
                    .s3_bucket_destination
                    .map(AnalyticsS3BucketDestination::from),
            )
            .build()
    }
}

impl From<SerializableAnalyticsS3BucketDestination> for AnalyticsS3BucketDestination {
    fn from(value: SerializableAnalyticsS3BucketDestination) -> Self {
        AnalyticsS3BucketDestinationBuilder::default()
            .set_bucket(Some(value.bucket))
            .set_format(Some(value.format.into()))
            .set_prefix(value.prefix)
            .set_bucket_account_id(value.bucket_account_id)
            .build()
            .unwrap()
    }
}

impl From<SerializableAnalyticsS3ExportFileFormat> for AnalyticsS3ExportFileFormat {
    fn from(value: SerializableAnalyticsS3ExportFileFormat) -> Self {
        match value {
            SerializableAnalyticsS3ExportFileFormat::Csv => Self::Csv,
            SerializableAnalyticsS3ExportFileFormat::Unknown => Self::Csv,
        }
    }
}

impl From<SerializableStorageClassAnalysisSchemaVersion> for StorageClassAnalysisSchemaVersion {
    fn from(value: SerializableStorageClassAnalysisSchemaVersion) -> Self {
        match value {
            SerializableStorageClassAnalysisSchemaVersion::V1 => Self::V1,
            SerializableStorageClassAnalysisSchemaVersion::Unknown => Self::V1,
        }
    }
}

impl From<SerializableAnalyticsFilter> for AnalyticsFilter {
    fn from(value: SerializableAnalyticsFilter) -> Self {
        match value {
            SerializableAnalyticsFilter::And(v) => Self::And(AnalyticsAndOperator::from(v)),
            SerializableAnalyticsFilter::Prefix(v) => Self::Prefix(v),
            SerializableAnalyticsFilter::Tag(v) => Self::Tag(Tag::from(v)),
            SerializableAnalyticsFilter::Unknown => Self::Unknown,
        }
    }
}

impl From<SerializableAnalyticsAndOperator> for AnalyticsAndOperator {
    fn from(value: SerializableAnalyticsAndOperator) -> Self {
        AnalyticsAndOperatorBuilder::default()
            .set_prefix(value.prefix)
            .set_tags(value.tags.map(|x| x.into_iter().map(Tag::from).collect()))
            .build()
    }
}

impl From<SerializableAccessControlPolicy> for AccessControlPolicy {
    fn from(value: SerializableAccessControlPolicy) -> Self {
        AccessControlPolicyBuilder::default()
            .set_grants(
                value
                    .grants
                    .map(|x| x.into_iter().map(Grant::from).collect()),
            )
            .set_owner(value.owner.map(Owner::from))
            .build()
    }
}

impl From<SerializableOwner> for Owner {
    fn from(value: SerializableOwner) -> Self {
        OwnerBuilder::default()
            .set_display_name(value.display_name)
            .set_id(value.id)
            .build()
    }
}

impl From<SerializableGrant> for Grant {
    fn from(value: SerializableGrant) -> Self {
        GrantBuilder::default()
            .set_grantee(value.grantee.map(Grantee::from))
            .set_permission(value.permission.map(Permission::from))
            .build()
    }
}

impl From<SerializablePermission> for Permission {
    fn from(value: SerializablePermission) -> Self {
        match value {
            SerializablePermission::FullControl => Self::FullControl,
            SerializablePermission::Read => Self::Read,
            SerializablePermission::ReadAcp => Self::ReadAcp,
            SerializablePermission::Write => Self::Write,
            SerializablePermission::WriteAcp => Self::WriteAcp,
            SerializablePermission::Unknown => Self::FullControl,
        }
    }
}

impl From<SerializableGrantee> for Grantee {
    fn from(value: SerializableGrantee) -> Self {
        GranteeBuilder::default()
            .set_display_name(value.display_name)
            .set_email_address(value.email_address)
            .set_id(value.id)
            .set_uri(value.uri)
            .set_type(Some(value.r#type.into()))
            .build()
            .unwrap()
    }
}

impl From<SerializableType> for Type {
    fn from(value: SerializableType) -> Self {
        match value {
            SerializableType::AmazonCustomerByEmail => Self::AmazonCustomerByEmail,
            SerializableType::CanonicalUser => Self::CanonicalUser,
            SerializableType::Group => Self::Group,
            SerializableType::Unknown => Self::AmazonCustomerByEmail,
        }
    }
}

impl From<SerializableChecksumAlgorithm> for ChecksumAlgorithm {
    fn from(value: SerializableChecksumAlgorithm) -> Self {
        match value {
            SerializableChecksumAlgorithm::Crc32 => Self::Crc32,
            SerializableChecksumAlgorithm::Crc32C => Self::Crc32C,
            SerializableChecksumAlgorithm::Sha1 => Self::Sha1,
            SerializableChecksumAlgorithm::Sha256 => Self::Sha256,
            SerializableChecksumAlgorithm::Unknown => Self::Crc32,
        }
    }
}

impl From<SerializableAccelerateConfiguration> for AccelerateConfiguration {
    fn from(value: SerializableAccelerateConfiguration) -> Self {
        AccelerateConfigurationBuilder::default()
            .set_status(value.status.map(BucketAccelerateStatus::from))
            .build()
    }
}

impl From<SerializableBucketAccelerateStatus> for BucketAccelerateStatus {
    fn from(value: SerializableBucketAccelerateStatus) -> Self {
        match value {
            SerializableBucketAccelerateStatus::Enabled => Self::Enabled,
            SerializableBucketAccelerateStatus::Suspended => Self::Suspended,
            SerializableBucketAccelerateStatus::Unknown => Self::Enabled,
        }
    }
}

impl From<SerializableBucketCannedAcl> for BucketCannedAcl {
    fn from(value: SerializableBucketCannedAcl) -> Self {
        match value {
            SerializableBucketCannedAcl::AuthenticatedRead => Self::AuthenticatedRead,
            SerializableBucketCannedAcl::Private => Self::Private,
            SerializableBucketCannedAcl::PublicRead => Self::PublicRead,
            SerializableBucketCannedAcl::PublicReadWrite => Self::PublicReadWrite,
            SerializableBucketCannedAcl::Unknown => Self::AuthenticatedRead,
        }
    }
}

impl From<SerializableCreateBucketConfiguration> for CreateBucketConfiguration {
    fn from(value: SerializableCreateBucketConfiguration) -> Self {
        CreateBucketConfigurationBuilder::default()
            .set_location_constraint(
                value
                    .location_constraint
                    .map(BucketLocationConstraint::from),
            )
            .set_bucket(value.bucket.map(BucketInfo::from))
            .set_location(value.location.map(LocationInfo::from))
            .build()
    }
}

impl From<SerializableLocationInfo> for LocationInfo {
    fn from(value: SerializableLocationInfo) -> Self {
        LocationInfoBuilder::default()
            .set_type(value.r#type.map(LocationType::from))
            .set_name(value.name)
            .build()
    }
}

impl From<SerializableLocationType> for LocationType {
    fn from(value: SerializableLocationType) -> Self {
        match value {
            SerializableLocationType::AvailabilityZone => Self::AvailabilityZone,
            SerializableLocationType::Unknown => Self::AvailabilityZone,
        }
    }
}

impl From<SerializableBucketInfo> for BucketInfo {
    fn from(value: SerializableBucketInfo) -> Self {
        BucketInfoBuilder::default()
            .set_data_redundancy(value.data_redundancy.map(DataRedundancy::from))
            .set_type(value.r#type.map(BucketType::from))
            .build()
    }
}

impl From<SerializableDataRedundancy> for DataRedundancy {
    fn from(value: SerializableDataRedundancy) -> Self {
        match value {
            SerializableDataRedundancy::SingleAvailabilityZone => Self::SingleAvailabilityZone,
            SerializableDataRedundancy::Unknown => Self::SingleAvailabilityZone,
        }
    }
}

impl From<SerializableBucketType> for BucketType {
    fn from(value: SerializableBucketType) -> Self {
        match value {
            SerializableBucketType::Directory => Self::Directory,
            SerializableBucketType::Unknown => Self::Directory,
        }
    }
}

impl From<SerializableBucketLocationConstraint> for BucketLocationConstraint {
    fn from(value: SerializableBucketLocationConstraint) -> Self {
        match value {
            SerializableBucketLocationConstraint::Eu => Self::Eu,
            SerializableBucketLocationConstraint::AfSouth1 => Self::AfSouth1,
            SerializableBucketLocationConstraint::ApEast1 => Self::ApEast1,
            SerializableBucketLocationConstraint::ApNortheast1 => Self::ApNortheast1,
            SerializableBucketLocationConstraint::ApNortheast2 => Self::ApNortheast2,
            SerializableBucketLocationConstraint::ApNortheast3 => Self::ApNortheast3,
            SerializableBucketLocationConstraint::ApSouth1 => Self::ApSouth1,
            SerializableBucketLocationConstraint::ApSouth2 => Self::ApSouth2,
            SerializableBucketLocationConstraint::ApSoutheast1 => Self::ApSoutheast1,
            SerializableBucketLocationConstraint::ApSoutheast2 => Self::ApSoutheast2,
            SerializableBucketLocationConstraint::ApSoutheast3 => Self::ApSoutheast3,
            SerializableBucketLocationConstraint::CaCentral1 => Self::CaCentral1,
            SerializableBucketLocationConstraint::CnNorth1 => Self::CnNorth1,
            SerializableBucketLocationConstraint::CnNorthwest1 => Self::CnNorthwest1,
            SerializableBucketLocationConstraint::EuCentral1 => Self::EuCentral1,
            SerializableBucketLocationConstraint::EuNorth1 => Self::EuNorth1,
            SerializableBucketLocationConstraint::EuSouth1 => Self::EuSouth1,
            SerializableBucketLocationConstraint::EuSouth2 => Self::EuSouth2,
            SerializableBucketLocationConstraint::EuWest1 => Self::EuWest1,
            SerializableBucketLocationConstraint::EuWest2 => Self::EuWest2,
            SerializableBucketLocationConstraint::EuWest3 => Self::EuWest3,
            SerializableBucketLocationConstraint::MeSouth1 => Self::MeSouth1,
            SerializableBucketLocationConstraint::SaEast1 => Self::SaEast1,
            SerializableBucketLocationConstraint::UsEast2 => Self::UsEast2,
            SerializableBucketLocationConstraint::UsGovEast1 => Self::UsGovEast1,
            SerializableBucketLocationConstraint::UsGovWest1 => Self::UsGovWest1,
            SerializableBucketLocationConstraint::UsWest1 => Self::UsWest1,
            SerializableBucketLocationConstraint::UsWest2 => Self::UsWest2,
            SerializableBucketLocationConstraint::Unknown => Self::UsEast2,
        }
    }
}
impl From<SerializableTagging> for Tagging {
    fn from(value: SerializableTagging) -> Self {
        TaggingBuilder::default()
            .set_tag_set(Some(value.tag_set.into_iter().map(Tag::from).collect()))
            .build()
            .unwrap()
    }
}

impl From<SerializableVersioningConfiguration> for VersioningConfiguration {
    fn from(value: SerializableVersioningConfiguration) -> Self {
        VersioningConfigurationBuilder::default()
            .set_status(value.status.map(BucketVersioningStatus::from))
            .build()
    }
}

impl From<SerializableWebsiteConfiguration> for WebsiteConfiguration {
    fn from(value: SerializableWebsiteConfiguration) -> Self {
        WebsiteConfigurationBuilder::default()
            .set_error_document(value.error_document.map(ErrorDocument::from))
            .set_index_document(value.index_document.map(IndexDocument::from))
            .set_redirect_all_requests_to(
                value
                    .redirect_all_requests_to
                    .map(RedirectAllRequestsTo::from),
            )
            .set_routing_rules(
                value
                    .routing_rules
                    .map(|x| x.into_iter().map(RoutingRule::from).collect()),
            )
            .build()
    }
}
