// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

#![cfg(target_os = "linux")]

use std::{collections::HashMap, path::Path, sync::Arc};

use core_model_builder::plugin::BuildMode;
use exo_env::MapEnvironment;
use serde_json::{Value, json};
use server_aws_lambda::resolve;
use server_common::create_static_loaders;
use system_router::create_system_router_from_system;

fn create_graphql_event(test_request: TestRequest<'_>) -> Value {
    let query_part = json!({
        "query": test_request.query,
        "variables": null
    });
    json!(
    {
        "cookies": test_request.cookies.into_iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<String>>(),
        "headers": Value::from(serde_json::Map::from_iter(test_request.headers.into_iter().map(|(k, v)| (k.to_string(), Value::from(v.to_string()))))),
        "requestContext": {
            "http": {
                "method": test_request.method.to_string(),
                "path": test_request.path,
                "sourceIp": test_request.ip
            }
        },
        "multiValueHeaders": null,
        "body": serde_json::to_string(&query_part).unwrap()
    })
}

pub struct TestRequest<'a> {
    pub query: &'a str,
    pub headers: HashMap<&'a str, &'a str>,
    pub ip: &'a str,
    pub cookies: Vec<(&'a str, &'a str)>,
    pub method: http::Method,
    pub path: &'a str,
}

pub struct TestResponse<'a> {
    pub body: Value,
    pub headers: HashMap<&'a str, &'a str>,
    pub cookies: Vec<(&'a str, &'a str)>,
    pub status_code: u16,
}

pub async fn test_query(test_request: TestRequest<'_>, expected: TestResponse<'_>) {
    let project_root = env!("CARGO_MANIFEST_DIR");
    let project_dir = Path::new(project_root).join("tests/test-model");

    std::env::set_current_dir(project_dir.clone()).expect(&format!(
        "Failed to set current directory to {}",
        project_dir.display()
    ));
    let model_path = project_dir.join("src/index.exo");

    let context = lambda_runtime::Context::default();

    let static_builders: Vec<
        Box<dyn core_plugin_interface::interface::SubsystemBuilder + Send + Sync>,
    > = vec![
        Box::new(postgres_builder::PostgresSubsystemBuilder::default()),
        Box::new(deno_builder::DenoSubsystemBuilder::default()),
        Box::new(wasm_builder::WasmSubsystemBuilder::default()),
    ];

    let model_system = builder::build_system(
        model_path,
        &builder::RealFileSystem,
        None::<&Path>,
        None,
        static_builders,
        BuildMode::Build,
    )
    .await
    .expect("Failed to build system");

    let system_router = create_system_router_from_system(
        model_system,
        create_static_loaders(),
        Arc::new(MapEnvironment::from([])),
    )
    .await
    .expect("Failed to create system router");

    let event = lambda_runtime::LambdaEvent::new(create_graphql_event(test_request), context);

    let result = resolve(event, Arc::new(system_router))
        .await
        .expect("Failed to resolve");

    let expected_multi_value_headers = expected
        .headers
        .into_iter()
        .chain(HashMap::from([("content-type", "application/json")]))
        .map(|(k, v)| {
            (
                k.to_string(),
                serde_json::Value::Array(vec![v.to_string().into()]),
            )
        });

    let expected_json = json!({
        "isBase64Encoded": false,
        "statusCode": expected.status_code,
        "headers": {},
        "multiValueHeaders": serde_json::Map::from_iter(expected_multi_value_headers),
        "cookies": expected.cookies.into_iter().map(|(k, v)| format!("{}={}", k, v)).collect::<Vec<_>>(),
        "body": expected.body
    });

    println!(
        "!! expected: {}",
        serde_json::to_string_pretty(&expected_json).unwrap()
    );
    println!(
        "!! actual: {}",
        serde_json::to_string_pretty(&result).unwrap()
    );

    assert_eq!(
        expected_json.as_object().unwrap().keys().len(),
        result.as_object().unwrap().keys().len()
    );
    for key in expected_json.as_object().unwrap().keys() {
        // Body comes as a string, so parse it into a JSON object to normalize spaces in it
        let (expected_value, actual_value) = if key == "body" {
            let expected_json_body: Value = expected_json["body"].clone();
            let actual_json_body: Value =
                serde_json::from_str(&result["body"].as_str().unwrap()).unwrap();

            (expected_json_body, actual_json_body)
        } else {
            (expected_json[key].clone(), result[key].clone())
        };

        assert_eq!(expected_value, actual_value, "Mismatch for key: {}", key);
    }
}
