use std::sync::Arc;

use resolver::create_system_resolver_from_serialized_bytes;
use serde_json::Value;
use server_aws_lambda::resolve;

pub async fn test_query(json_input: Value, clay_model: &str, expected: Value) {
    let context = lambda_runtime::Context::default();
    let event = lambda_runtime::LambdaEvent::new(json_input, context);

    // HACK: some envvars need to be set to create a SystemContext
    {
        std::env::set_var("CLAY_CHECK_CONNECTION_ON_STARTUP", "false");
        std::env::set_var("CLAY_DATABASE_URL", "postgres://a@dummy-value");
    }

    let model_system =
        builder::build_system_from_str(clay_model, "index.clay".to_string()).unwrap();
    let system_resolver =
        Arc::new(create_system_resolver_from_serialized_bytes(model_system).unwrap());

    let result = resolve(event, system_resolver).await.unwrap();

    println!(
        "!! expected: {}",
        serde_json::to_string_pretty(&expected).unwrap()
    );
    println!(
        "!! actual: {}",
        serde_json::to_string_pretty(&result).unwrap()
    );

    assert_eq!(expected, result)
}
