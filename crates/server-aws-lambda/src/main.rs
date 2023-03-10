use lambda_runtime::Error;
use lambda_runtime::LambdaEvent;

use serde_json::Value;
use server_aws_lambda::resolve;

use std::sync::Arc;

/// Run the server in production mode with a compiled claypot file
#[tokio::main]
async fn main() -> Result<(), Error> {
    let system_resolver = Arc::new(server_common::init());

    let service = lambda_runtime::service_fn(|event: LambdaEvent<Value>| async {
        resolve(event, system_resolver.clone()).await
    });

    lambda_runtime::run(service).await?;

    Ok(())
}
