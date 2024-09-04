use std::env;

use anyhow::{anyhow, Context};
use api::integration::RestAPIIntegration;
use rscloud::prelude::*;
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
    let region = "us-east-2";
    let aws = cloud.aws_provider(handle.clone(), region);

    let functions = rust_lambda_functions(
        &aws,
        RustLambdaInput {
            assume_role: aws
                .resource::<Role>("lambda_assume_role", Present, Default::default())?
                .for_service("lambda.amazonaws.com")?,
            functions: vec![RustLambdaEntry::new("rscloud", "sample_echo1")],
        },
    )?;

    let rest_api: RestAPI = aws.resource::<RestAPI>(
        "myapi",
        Present,
        RestAPIInput {
            name: Some("myapi".to_string()),
            description: Some("My API".to_string()),
            ..Default::default()
        },
    )?;

    let echo_endpoint: RestAPIResource =
        aws.resource::<RestAPIResource>("myapi_echo", Present, Default::default())?;

    echo_endpoint.bind(&rest_api, |me, other| {
        me.parent_id = other.root_resource_id.clone();
        me.rest_api_id = other.id.clone();
        me.path_part = Some("echo".to_string());
    })?;

    let echo_integration =
        aws.resource::<RestAPIIntegration>("myapi_echo_integration", Present, Default::default())?;

    echo_integration.bind(&echo_endpoint, |me, other| {
        me.rest_api_id = other.rest_api_id.clone();
        me.resource_id = other.id.clone();
        me.http_method = Some("POST".to_string());
        me.r#type = Some(SerializableIntegrationType::AwsProxy);
    })?;

    echo_integration.bind(
        &functions.functions.get("sample_echo1").unwrap().function,
        move |me, other| {
            me.uri = Some(format!(
                "arn:aws:apigateway:{}:lambda:path/2015-03-31/functions/{}/invocations",
                region,
                other.arn().unwrap()[0]
            ));
        },
    )?;

    Ok(cloud)
}
