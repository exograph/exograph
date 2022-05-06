use std::path::Path;

use deno_core::error::AnyError;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::{op, Extension};
use futures::future::join_all;
use payas_deno::{Arg, DenoActor, DenoExecutor, DenoModule, DenoModuleSharedState, UserCode};
use tokio::sync::mpsc::channel;

#[tokio::test]
async fn test_direct_sync() {
    let mut deno_module = DenoModule::new(
        UserCode::LoadFromFs(Path::new("./tests/direct.js").to_owned()),
        "deno_module",
        &[],
        vec![],
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

#[tokio::test]
async fn test_actor() {
    let mut actor = DenoActor::new(
        UserCode::LoadFromFs(Path::new("./tests/direct.js").to_path_buf()),
        DenoModuleSharedState::default(),
    )
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

#[tokio::test]
async fn test_actor_executor() {
    let executor = DenoExecutor::default();

    let module_path = "tests/direct.js";
    let module_script = include_str!("./direct.js");

    executor
        .preload_module(module_path, module_script, 1)
        .await
        .unwrap();

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

#[tokio::test]
async fn test_actor_executor_concurrent() {
    let executor = DenoExecutor::default();
    let module_path = "tests/direct.js";
    let module_script = include_str!("./direct.js");
    let total_futures = 10;

    // start with one preloaded DenoModule
    executor
        .preload_module(module_path, include_str!("./direct.js"), 1)
        .await
        .unwrap();

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

#[tokio::test]
async fn test_direct_async() {
    let mut deno_module = DenoModule::new(
        UserCode::LoadFromFs(Path::new("./tests/direct.js").to_owned()),
        "deno_module",
        &[],
        vec![],
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

#[tokio::test]
async fn test_shim_sync() {
    static GET_JSON_SHIM: (&str, &str) = ("__shim", include_str!("shim.js"));

    let mut deno_module = DenoModule::new(
        UserCode::LoadFromFs(Path::new("./tests/through_shim.js").to_owned()),
        "deno_module",
        &[GET_JSON_SHIM],
        vec![],
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

#[tokio::test]
async fn test_shim_async() {
    static GET_JSON_SHIM: (&str, &str) = ("__shim", include_str!("shim.js"));

    let mut deno_module = DenoModule::new(
        UserCode::LoadFromFs(Path::new("./tests/through_shim.js").to_owned()),
        "deno_module",
        &[GET_JSON_SHIM],
        vec![],
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

#[op]
fn rust_impl(arg: String) -> Result<String, AnyError> {
    Ok(format!("Register Op: {}", arg))
}

#[tokio::test]
async fn test_register_ops() {
    let mut deno_module = DenoModule::new(
        UserCode::LoadFromFs(Path::new("./tests/through_rust_fn.js").to_owned()),
        "deno_module",
        &[],
        vec![Extension::builder().ops(vec![rust_impl::decl()]).build()],
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
