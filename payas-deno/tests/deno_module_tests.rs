use deno_core::{serde_json::Value, JsRuntime};
use payas_deno::{Arg, DenoModule};

use deno_core::serde_json::json;

#[tokio::test]
async fn test_basic() {
    let mut deno_module = DenoModule::new("./tests/basic.js", "deno_module", &[], |_| {})
        .await
        .unwrap();

    let sync_ret_value = deno_module
        .execute_function(
            "addAndDouble",
            vec![
                Arg::Serde(Value::Number(42.into())),
                Arg::Serde(Value::Number(21.into())),
            ],
        )
        .await
        .unwrap();

    assert_eq!(sync_ret_value, Value::Number(126.into()));

    let async_ret_value = deno_module
        .execute_function("getJson", vec![Arg::Serde(Value::String("4".into()))])
        .await
        .unwrap();
    assert_eq!(
        async_ret_value,
        json!({ "userId": 1, "id": 4, "title": "et porro tempora", "completed": true })
    );
}

#[tokio::test]
async fn test_shim() {
    static GET_JSON_SHIM: (&str, &str) = ("__get_json_shim", include_str!("get_json_shim.js"));

    let mut deno_module =
        DenoModule::new("./tests/basic.js", "deno_module", &[GET_JSON_SHIM], |_| {})
            .await
            .unwrap();

    let sync_ret_value = deno_module
        .execute_function(
            "syncUsingShim",
            vec![
                Arg::Serde(Value::String("param".into())),
                Arg::Shim("__get_json_shim".to_string()),
            ],
        )
        .await
        .unwrap();
    assert_eq!(sync_ret_value, Value::String("value: param".into()));

    let async_ret_value = deno_module
        .execute_function(
            "asyncUsingShim",
            vec![
                Arg::Serde(Value::String("4".into())),
                Arg::Shim("__get_json_shim".to_string()),
            ],
        )
        .await
        .unwrap();
    assert_eq!(
        async_ret_value,
        json!({ "userId": 1, "id": 4, "title": "et porro tempora", "completed": true })
    );
}

#[tokio::test]
async fn test_register_ops() {
    static GET_JSON_SHIM: (&str, &str) = ("__get_json_shim", include_str!("get_json_shim.js"));

    fn register_ops(runtime: &mut JsRuntime) {
        let sync_ops = vec![(
            "rust_impl",
            deno_core::op_sync(|_state, args: Vec<String>, _: ()| {
                Ok(format!("Register Op: {}", args[0]))
            }),
        )];
        for (name, op) in sync_ops {
            runtime.register_op(name, op);
        }
    }

    let mut deno_module = DenoModule::new(
        "./tests/basic.js",
        "deno_module",
        &[GET_JSON_SHIM],
        register_ops,
    )
    .await
    .unwrap();

    let sync_ret_value = deno_module
        .execute_function(
            "syncUsingRegisteredFunction",
            vec![
                Arg::Serde(Value::String("param".into())),
                Arg::Shim("__get_json_shim".to_string()),
            ],
        )
        .await
        .unwrap();
    assert_eq!(sync_ret_value, Value::String("Register Op: param".into()));
}
