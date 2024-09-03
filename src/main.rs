use std::env;

use anyhow::{anyhow, Context};
use rscloud::{
    prelude::*,
    v1::solutions::{self},
};
use serde_json::json;
fn main() -> anyhow::Result<()> {
    let args: Vec<String> = env::args().collect();
    let rt = tokio::runtime::Runtime::new()?;
    let mut cloud = create_cloud(rt.handle())?;
    if args.len() < 2 {
        print_usage(&args[0]);
        return Err(anyhow!("No arguments has been provided"));
    }
    match args[1].as_str() {
        "apply" => cloud
            .apply()
            .context("Could now apply cloud infrastructure"),
        "destroy" => cloud
            .destroy()
            .context("Could not destroy cloud infrastructure"),
        other => {
            print_usage(&args[0]);
            Err(anyhow!("Invalid command: {}", other))
        }
    }
}
fn print_usage(cmd: &str) {
    println!("Usage: {} <command>", cmd);
    println!("Commands:");
    println!("  apply    Apply the configuration");
    println!("  destroy  Destroy the configuration");
}

fn create_cloud(handle: &tokio::runtime::Handle) -> anyhow::Result<FPCloud> {
    let mut cloud = FPCloud::default();
    cloud.init_registry(handle.clone());
    let aws = cloud.aws_provider(handle.clone(), "us-east-2");
    let vpc = aws.resource::<Vpc>(
        "my_vpc",
        Present,
        VpcInput {
            cidr_block: Some("10.0.0.0/16".to_string()),
            ..Default::default()
        },
    )?;
    let _subnets = vpc.subnets(vec![
        SubnetInput {
            //availability_zone: ,
            ..Default::default()
        },
        SubnetInput {
            ..Default::default()
        },
        SubnetInput {
            ..Default::default()
        },
        SubnetInput {
            ..Default::default()
        },
    ])?;

    let rust_lambda_functions = solutions::lambda::rust_lambda_functions(
        &aws,
        solutions::lambda::RustLambdaInput {
            assume_role: aws
                .resource::<Role>("lambda_assume_role", Present, Default::default())?
                .for_service("lambda.amazonaws.com")?,
            functions: vec![RustLambdaEntry::new("rscloud", "sample_echo1")],
        },
    )?;
    let bucket = aws.resource::<Bucket>(
        "my_bucket",
        Present,
        BucketInput {
            bucket: SerializableCreateBucketInput {
                bucket: Some("fpco-test-bucket".to_owned()),
                create_bucket_configuration: Some(SerializableCreateBucketConfiguration {
                    location_constraint: Some(SerializableBucketLocationConstraint::UsEast2),
                    ..Default::default()
                }),
                ..Default::default()
            },
            ..Default::default()
        },
    )?;
    let file = aws.resource::<S3Object>(
        "my_file",
        Present,
        S3ObjectInput {
            key: Some("my_file".to_string()),
            body: "Hello, world!".to_string().into_bytes(),
            ..Default::default()
        },
    )?;
    file.bind_bucket(&bucket)?;
    let state_machine_role = aws
        .resource::<Role>("state-machine_role", Present, Default::default())?
        .for_service("states.amazonaws.com")?;
    rust_lambda_functions.grant_permission(&state_machine_role)?;
    let _state_machine = aws
        .resource::<StateMachine>(
            "my_state_machine",
            Present,
            SerializableCreateStateMachineInput {
                name: Some("my_state_machine".to_string()),
                definition: Some(StateMachineDocument {
                    comment: Some("Execute Lambda functions".to_string()),
                    start_at: "FirstFunction".to_string(),
                    states: vec![
                        (
                            String::from("FirstFunction"),
                            State::Task(TaskState {
                                next: Some("SecondFunction".to_string()),
                                end: None,
                                result_path: Some("$.data.first.output".to_string()),
                                timeout_seconds: 300,
                                heartbeat_seconds: 99999999,
                                parameters: Some(json!({
                                    "echo": "Echo"
                                })),
                                ..Default::default()
                            }),
                        ),
                        (
                            String::from("SecondFunction"),
                            State::Task(TaskState {
                                next: Some("ThirdFunction".to_string()),
                                end: None,
                                result_path: Some("$.data.second.output".to_string()),
                                timeout_seconds: 300,
                                heartbeat_seconds: 99999999,
                                parameters: Some(json!({
                                    "echo.$": "$.data.first.output.body"
                                })),
                                ..Default::default()
                            }),
                        ),
                        (
                            String::from("ThirdFunction"),
                            State::Task(TaskState {
                                result_path: Some("$.data.third.output".to_string()),
                                next: None,
                                end: Some(true),
                                timeout_seconds: 300,
                                heartbeat_seconds: 99999999,
                                parameters: Some(json!({
                                    "echo.$": "$.data.second.output.body"
                                })),
                                ..Default::default()
                            }),
                        ),
                    ]
                    .into_iter()
                    .collect(),
                }),
                r#type: Some(SerializableStateMachineType::Standard),
                logging_configuration: Some(SerializableLoggingConfigurationForSM {
                    level: Some(SerializableLogLevelForSM::All),
                    include_execution_data: true,
                    destinations: None,
                }),
                ..Default::default()
            },
        )?
        .bind_role(&state_machine_role)?
        .bind_function(
            "FirstFunction",
            &rust_lambda_functions.functions["sample_echo1"].function,
        )?
        .bind_function(
            "SecondFunction",
            &rust_lambda_functions.functions["sample_echo1"].function,
        )?
        .bind_function(
            "ThirdFunction",
            &rust_lambda_functions.functions["sample_echo1"].function,
        )?;

    /*
    {
      "data": {
        "input": {
            "echo": "Echo"
        }
      }
    }

             */

    Ok(cloud)
}
