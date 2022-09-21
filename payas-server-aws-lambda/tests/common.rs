use std::sync::Arc;

use payas_server_aws_lambda::resolve;
use payas_server_core::create_system_context_with_model_system;
use serde_json::Value;

pub async fn test_query(json_input: Value, clay_model: &str, expected: Value) {
    let context = lambda_runtime::Context::default();
    let event = lambda_runtime::LambdaEvent::new(json_input, context);

    let model_system =
        payas_parser::build_system_from_str(clay_model, "index.clay".to_string()).unwrap();
    let system_context = Arc::new(create_system_context_with_model_system(model_system).unwrap());

    // HACK: the envvars CLAY_INTROSPECTION and CLAY_DATABASE_URL need to be set to resolve anything
    {
        std::env::set_var("CLAY_INTROSPECTION", "true");
        std::env::set_var("CLAY_DATABASE_URL", "dummy-value");
    }

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
