use std::{
    path::Path,
    sync::{Arc, Mutex},
};

use anyhow::Result;
use deno_core::{serde_json::Value, JsRuntime};
use payas_deno::{Arg, DenoModule, DenoModulesMap};

use deno_core::serde_json::json;

fn no_op(_: String, _: Option<&serde_json::Map<String, Value>>) -> Result<serde_json::Value> {
    panic!()
}

#[tokio::test]
async fn test_direct_sync() {
    let mut deno_module =
        DenoModule::new(Path::new("./tests/direct.js"), "deno_module", &[], &|_| {})
            .await
            .unwrap();

    deno_module.preload_function(vec!["addAndDouble"]);

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
async fn test_module_map_threaded() {
    let deno_modules_map = Arc::new(Mutex::new(DenoModulesMap::new()));

    {
        deno_modules_map
            .lock()
            .unwrap()
            .load_module(Path::new("./tests/direct.js"))
            .unwrap();
    }

    let mut handles = vec![];

    for _ in 1..=10 {
        let deno_modules_map = deno_modules_map.clone();

        let handle = std::thread::spawn(move || {
            let ret_value = deno_modules_map
                .lock()
                .unwrap()
                .execute_function(
                    Path::new("./tests/direct.js"),
                    "addAndDouble",
                    vec![
                        Arg::Serde(Value::Number(4.into())),
                        Arg::Serde(Value::Number(2.into())),
                    ],
                    &no_op,
                )
                .unwrap();

            assert_eq!(ret_value, Value::Number(12.into()));
        });

        handles.push(handle);
    }

    for handle in handles.into_iter() {
        handle.join().unwrap()
    }
}

#[tokio::test]
async fn test_direct_async() {
    let mut deno_module =
        DenoModule::new(Path::new("./tests/direct.js"), "deno_module", &[], &|_| {})
            .await
            .unwrap();

    deno_module.preload_function(vec!["getJson"]);

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
        Path::new("./tests/through_shim.js"),
        "deno_module",
        &[GET_JSON_SHIM],
        &|_| {},
    )
    .await
    .unwrap();

    deno_module.preload_function(vec!["addAndDoubleThroughShim"]);

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
        Path::new("./tests/through_shim.js"),
        "deno_module",
        &[GET_JSON_SHIM],
        &|_| {},
    )
    .await
    .unwrap();

    deno_module.preload_function(vec!["getJsonThroughShim"]);

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

#[tokio::test]
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
    )
    .await
    .unwrap();

    deno_module.preload_function(vec!["syncUsingRegisteredFunction"]);

    let sync_ret_value = deno_module
        .execute_function(
            "syncUsingRegisteredFunction",
            vec![Arg::Serde(Value::String("param".into()))],
        )
        .await
        .unwrap();
    assert_eq!(sync_ret_value, Value::String("Register Op: param".into()));
}
