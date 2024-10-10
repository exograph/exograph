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

use exo_env::MapEnvironment;
use router::system_router::create_system_router_from_system;
use serde_json::{json, Value};
use server_aws_lambda::resolve;
use server_common::create_static_loaders;

fn create_graphql_event(
    query: &str,
    headers: HashMap<&str, &str>,
    source_ip: &str,
    cookies: Vec<(&str, &str)>,
    method: http::Method,
    path: &str,
) -> Value {
    let query_part = json!({
        "query": query,
        "variables": null
    });
    json!(
    {
        "cookies": cookies.into_iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<String>>(),
        "headers": Value::from(serde_json::Map::from_iter(headers.into_iter().map(|(k, v)| (k.to_string(), Value::from(v.to_string()))))),
        "requestContext": {
            "http": {
                "method": method.to_string(),
                "path": path,
                "sourceIp": source_ip
            }
        },
        "multiValueHeaders": null,
        "body": serde_json::to_string(&query_part).unwrap()
    })
}

pub async fn test_query(
    query: &str,
    request_headers: HashMap<&str, &str>,
    source_ip: &str,
    cookies: Vec<(&str, &str)>,
    method: http::Method,
    path: &str,
    expected: Value,
) {
    let current_dir = std::env::current_dir().unwrap();
    let project_dir = current_dir.join("tests/test-model");

    std::env::set_current_dir(project_dir.clone()).unwrap();
    let model_path = project_dir.join("src/index.exo");

    let context = lambda_runtime::Context::default();

    let model_system = builder::build_system(model_path, None::<&Path>, vec![])
        .await
        .unwrap();

    let system_router = create_system_router_from_system(
        model_system,
        create_static_loaders(),
        Arc::new(MapEnvironment::from([])),
    )
    .await
    .unwrap();

    let event = lambda_runtime::LambdaEvent::new(
        create_graphql_event(&query, request_headers, source_ip, cookies, method, path),
        context,
    );

    let result = resolve(event, Arc::new(system_router)).await.unwrap();

    println!(
        "!! expected: {}",
        serde_json::to_string_pretty(&expected).unwrap()
    );
    println!(
        "!! actual: {}",
        serde_json::to_string_pretty(&result).unwrap()
    );

    assert_eq!(
        expected.as_object().unwrap().keys().len(),
        result.as_object().unwrap().keys().len()
    );
    for key in expected.as_object().unwrap().keys() {
        // Body comes as a string, so parse it into a JSON object to normalize spaces in it
        let (expected_value, actual_value) = if key == "body" {
            let expected_json_body: Value =
                serde_json::from_str(&expected["body"].as_str().unwrap()).unwrap();
            let actual_json_body: Value =
                serde_json::from_str(&result["body"].as_str().unwrap()).unwrap();

            (expected_json_body, actual_json_body)
        } else {
            (expected[key].clone(), result[key].clone())
        };

        assert_eq!(expected_value, actual_value, "Mismatch for key: {}", key);
    }

    std::env::set_current_dir(current_dir).unwrap();
}
