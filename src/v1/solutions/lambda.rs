use std::collections::HashMap;

use crate::prelude::*;
use anyhow::Context;

pub struct RustLambdaOutput<'a> {
    pub functions: HashMap<String, RustLambda<'a>>,
}

impl<'a> RustLambdaOutput<'a> {
    pub fn grant_permission(&self, role: &Role) -> anyhow::Result<()> {
        self.functions
            .iter()
            .try_for_each(|(_, f)| -> anyhow::Result<()> {
                role.attach_policy(&f.allow_policy)?;
                Ok(())
            })
    }
    pub fn grant_permission_to_user(&self, user: &IAMUser) -> anyhow::Result<()> {
        self.functions
            .iter()
            .try_for_each(|(_, f)| -> anyhow::Result<()> {
                user.attach_policy(&f.allow_policy)?;
                Ok(())
            })
    }
}
pub struct RustLambdaInput<'a> {
    pub assume_role: Role<'a>,
    pub functions: Vec<&'static str>,
}
pub struct RustLambda<'a> {
    pub function: Function<'a>,
    pub allow_policy: Policy<'a>,
}

pub fn rust_lambda_functions<'a>(
    aws: &'a AwsProvider,
    input: RustLambdaInput,
) -> anyhow::Result<RustLambdaOutput<'a>> {
    //Generate error if functions are empty
    if input.functions.is_empty() {
        return Err(anyhow::anyhow!("No function to build"));
    }
    input.assume_role.attach_managed_policy(
        "arn:aws:iam::aws:policy/service-role/AWSLambdaBasicExecutionRole",
    )?;
    //Create the lambda rust functions
    input
        .functions
        .into_iter()
        .try_fold(
            HashMap::new(),
            |mut acc, function_name| -> anyhow::Result<HashMap<String, RustLambda>> {
                let function_input = FunctionInput {
                    runtime: Some(SerializableRuntime::Providedal2023),
                    handler: Some("main:handler".to_string()),
                    code: Some(SerializableFunctionCode {
                        zip_file: Some(get_cargo_lambda_function_code(function_name)?),
                        ..Default::default()
                    }),
                    ..Default::default()
                };
                let function = aws
                    .resource::<Function>(function_name, Present, function_input)
                    .context("Fail to create function")?;
                function
                    .bind_role(&input.assume_role)
                    .context("Fail to bind role to function")?;
                let policy = aws.resource::<Policy>(
                    &format!("{}-allow", function_name),
                    Present,
                    Default::default(),
                )?;
                policy.configure("Allow", vec!["lambda:InvokeFunction"], &function)?;
                acc.insert(
                    function_name.to_string(),
                    RustLambda {
                        function,
                        allow_policy: policy,
                    },
                );
                Ok(acc)
            },
        )
        .map(|functions| RustLambdaOutput { functions })
        .context("Fail to add lambda functions")
}
