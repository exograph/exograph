use std::path::Path;

// we use Tokio 0.2 in order to work with actix-web's runtime but Deno requires Tokio 1.x
// this dance is needed to get macros for Tokio 1.x working
extern crate tokio as tokio_0_2;
extern crate tokio_1 as tokio;

use anyhow::Result;
use deno_core::serde_json::json;
use deno_core::{serde_json::Value, JsRuntime};
use futures::future::join_all;
use payas_deno::{Arg, DenoActor, DenoExecutor, DenoModule, DenoModuleSharedState, UserCode};
use tokio_0_2::sync::mpsc::channel;

fn no_op(_: String, _: Option<&serde_json::Map<String, Value>>) -> Result<serde_json::Value> {
    panic!()
}

#[tokio_1::test]
async fn test_direct_sync() {
    let mut deno_module = DenoModule::new(
        UserCode::LoadFromFs(Path::new("./tests/direct.js").to_owned()),
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

#[tokio_1::test]
async fn test_actor() {
    let mut actor = DenoActor::new(
        UserCode::LoadFromFs(Path::new("./tests/direct.js").to_path_buf()),
        DenoModuleSharedState::default(),
    )
    .await
    .unwrap();

    let (to_user_sender, _to_user_receiver) = channel(1);

    let res = actor
        .call_method(
            "addAndDouble".to_string(),
            vec![Arg::Serde(2.into()), Arg::Serde(3.into())],
            None,
            to_user_sender,
        )
        .await;

    assert_eq!(res.unwrap(), 10);
}

#[tokio_1::test]
async fn test_actor_executor() {
    let executor = DenoExecutor::default();

    let module_path = "./tests/direct.js";
    let module_script = include_str!("./direct.js");

    executor.preload_module(module_path, module_script, 1).await.unwrap();

    let res = executor
        .execute_function(
            module_path,
            module_script,
            "addAndDouble",
            vec![Arg::Serde(2.into()), Arg::Serde(3.into())],
        )
        .await;

    assert_eq!(res.unwrap(), 10);
}

#[tokio_1::test]
async fn test_actor_executor_concurrent() {
    let executor = DenoExecutor::default();
    let module_path = "./tests/direct.js";
    let module_script = include_str!("./direct.js");
    let total_futures = 10;

    // start with one preloaded DenoModule
    executor.preload_module(module_path, include_str!("./direct.js"), 1).await.unwrap();

    let mut handles = vec![];

    for _ in 1..=total_futures {
        let handle = executor.execute_function(
            module_path,
            module_script,
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

#[tokio_1::test]
async fn test_direct_async() {
    let mut deno_module = DenoModule::new(
        UserCode::LoadFromFs(Path::new("./tests/direct.js").to_owned()),
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

#[tokio_1::test]
async fn test_shim_sync() {
    static GET_JSON_SHIM: (&str, &str) = ("__shim", include_str!("shim.js"));

    let mut deno_module = DenoModule::new(
        UserCode::LoadFromFs(Path::new("./tests/through_shim.js").to_owned()),
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

#[tokio_1::test]
async fn test_shim_async() {
    static GET_JSON_SHIM: (&str, &str) = ("__shim", include_str!("shim.js"));

    let mut deno_module = DenoModule::new(
        UserCode::LoadFromFs(Path::new("./tests/through_shim.js").to_owned()),
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

#[tokio_1::test]
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
        UserCode::LoadFromFs(Path::new("./tests/through_rust_fn.js").to_owned()),
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
