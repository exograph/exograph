use std::sync::Arc;

use payas_server_aws_lambda::resolve;
use payas_server_core::create_system_resolver_from_serialized_bytes;
use serde_json::Value;

pub async fn test_query(json_input: Value, clay_model: &str, expected: Value) {
    let context = lambda_runtime::Context::default();
    let event = lambda_runtime::LambdaEvent::new(json_input, context);

    // HACK: some envvars need to be set to create a SystemContext
    {
        std::env::set_var("CLAY_CHECK_CONNECTION_ON_STARTUP", "false");
        std::env::set_var("CLAY_DATABASE_URL", "postgres://a@dummy-value");
    }

    let model_system =
        payas_builder::build_system_from_str(clay_model, "index.clay".to_string()).unwrap();
    let system_context =
        Arc::new(create_system_resolver_from_serialized_bytes(model_system).unwrap());

    let result = resolve(event, system_context).await.unwrap();

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
