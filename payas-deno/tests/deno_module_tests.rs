use std::path::Path;

use anyhow::Result;
use deno_core::serde_json::json;
use deno_core::{serde_json::Value, JsRuntime};
use futures::future::join_all;
use payas_deno::{Arg, DenoActor, DenoExecutor, DenoModule, DenoModuleSharedState, MethodCall};
use tokio::sync::mpsc::channel;

fn no_op(_: String, _: Option<&serde_json::Map<String, Value>>) -> Result<serde_json::Value> {
    panic!()
}

#[actix_rt::test]
async fn test_direct_sync() {
    let mut deno_module = DenoModule::new(
        Path::new("./tests/direct.js"),
        "deno_module",
        &[],
        |_| {},
        DenoModuleSharedState::default(),
    )
    .await
    .unwrap();

    let sync_ret_value = deno_module
        .execute_function(
            "addAndDouble",
            vec![
                Arg::Serde(Value::Number(4.into())),
                Arg::Serde(Value::Number(2.into())),
            ],
        )
        .await
        .unwrap();

    assert_eq!(sync_ret_value, Value::Number(12.into()));
}

#[actix_rt::test]
async fn test_actor() {
    let mut actor = DenoActor::new(
        Path::new("./tests/direct.js"),
        DenoModuleSharedState::default(),
    )
    .await;

    let (to_user_sender, _to_user_receiver) = channel(1);

    let res = actor
        .handle(MethodCall {
            method_name: "addAndDouble".to_string(),
            arguments: vec![Arg::Serde(2.into()), Arg::Serde(3.into())],
            to_user: to_user_sender,
        })
        .await;

    assert_eq!(res.unwrap(), 10);
}

#[actix_rt::test]
async fn test_actor_executor() {
    let executor = DenoExecutor::default();
    let module_path = Path::new("./tests/direct.js");

    executor.preload_module(module_path, 1).await;

    let res = executor
        .execute_function(
            module_path,
            "addAndDouble",
            vec![Arg::Serde(2.into()), Arg::Serde(3.into())],
        )
        .await;

    assert_eq!(res.unwrap(), 10);
}

#[tokio::test]
async fn test_actor_executor_concurrent() {
    let executor = DenoExecutor::default();
    let module_path = Path::new("./tests/direct.js");
    let total_futures = 10;

    // start with one preloaded DenoModule
    executor.preload_module(module_path, 1).await;

    let mut handles = vec![];

    for _ in 1..=total_futures {
        let handle = executor.execute_function(
            module_path,
            "addAndDouble",
            vec![
                Arg::Serde(Value::Number(4.into())),
                Arg::Serde(Value::Number(2.into())),
            ],
        );

        handles.push(handle);
    }

    let result = join_all(handles)
        .await
        .iter()
        .filter(|res| res.as_ref().unwrap() == 12)
        .count();

    assert_eq!(result, total_futures);
}

#[actix_rt::test]
async fn test_direct_async() {
    let mut deno_module = DenoModule::new(
        Path::new("./tests/direct.js"),
        "deno_module",
        &[],
        |_| {},
        DenoModuleSharedState::default(),
    )
    .await
    .unwrap();

    let async_ret_value = deno_module
        .execute_function("getJson", vec![Arg::Serde(Value::String("4".into()))])
        .await
        .unwrap();
    assert_eq!(
        async_ret_value,
        json!({ "userId": 1, "id": 4, "title": "et porro tempora", "completed": true })
    );

    // The JS side doesn't care if the id is a string or a number, so let's use number here
    let async_ret_value = deno_module
        .execute_function("getJson", vec![Arg::Serde(Value::Number(5.into()))])
        .await
        .unwrap();
    assert_eq!(
        async_ret_value,
        json!({ "userId": 1, "id": 5, "title": "laboriosam mollitia et enim quasi adipisci quia provident illum", "completed": false })
    );
}

#[actix_rt::test]
async fn test_shim_sync() {
    static GET_JSON_SHIM: (&str, &str) = ("__shim", include_str!("shim.js"));

    let mut deno_module = DenoModule::new(
        Path::new("./tests/through_shim.js"),
        "deno_module",
        &[GET_JSON_SHIM],
        |_| {},
        DenoModuleSharedState::default(),
    )
    .await
    .unwrap();

    let sync_ret_value = deno_module
        .execute_function(
            "addAndDoubleThroughShim",
            vec![
                Arg::Serde(Value::Number(4.into())),
                Arg::Serde(Value::Number(5.into())),
                Arg::Shim("__shim".to_string()),
            ],
        )
        .await
        .unwrap();
    assert_eq!(sync_ret_value, Value::Number(18.into()));

    let sync_ret_value = deno_module
        .execute_function(
            "addAndDoubleThroughShim",
            vec![
                Arg::Serde(Value::Number(42.into())),
                Arg::Serde(Value::Number(21.into())),
                Arg::Shim("__shim".to_string()),
            ],
        )
        .await
        .unwrap();
    assert_eq!(sync_ret_value, Value::Number(126.into()));
}

#[actix_rt::test]
async fn test_shim_async() {
    static GET_JSON_SHIM: (&str, &str) = ("__shim", include_str!("shim.js"));

    let mut deno_module = DenoModule::new(
        Path::new("./tests/through_shim.js"),
        "deno_module",
        &[GET_JSON_SHIM],
        |_| {},
        DenoModuleSharedState::default(),
    )
    .await
    .unwrap();

    let async_ret_value = deno_module
        .execute_function(
            "getJsonThroughShim",
            vec![
                Arg::Serde(Value::String("4".into())),
                Arg::Shim("__shim".to_string()),
            ],
        )
        .await
        .unwrap();
    assert_eq!(
        async_ret_value,
        json!({ "userId": 1, "id": 4, "title": "et porro tempora", "completed": true })
    );

    // The JS side doesn't care if the id is a string or a number, so let's use number here
    let async_ret_value = deno_module
        .execute_function(
            "getJsonThroughShim",
            vec![
                Arg::Serde(Value::Number(5.into())),
                Arg::Shim("__shim".to_string()),
            ],
        )
        .await
        .unwrap();
    assert_eq!(
        async_ret_value,
        json!({ "userId": 1, "id": 5, "title": "laboriosam mollitia et enim quasi adipisci quia provident illum", "completed": false })
    );
}

#[actix_rt::test]
async fn test_register_ops() {
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
        Path::new("./tests/through_rust_fn.js"),
        "deno_module",
        &[],
        &register_ops,
        DenoModuleSharedState::default(),
    )
    .await
    .unwrap();

    let sync_ret_value = deno_module
        .execute_function(
            "syncUsingRegisteredFunction",
            vec![Arg::Serde(Value::String("param".into()))],
        )
        .await
        .unwrap();
    assert_eq!(sync_ret_value, Value::String("Register Op: param".into()));
}
