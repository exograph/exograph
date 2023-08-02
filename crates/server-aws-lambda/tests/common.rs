// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

#![cfg(target_os = "linux")]

use std::sync::Arc;

use resolver::create_system_resolver_from_serialized_bytes;
use serde_json::Value;
use server_aws_lambda::resolve;
use server_common::create_static_loaders;

pub async fn test_query(json_input: Value, exo_model: &str, expected: Value) {
    let context = lambda_runtime::Context::default();
    let event = lambda_runtime::LambdaEvent::new(json_input, context);

    // HACK: some envvars need to be set to create a SystemContext
    {
        std::env::set_var("EXO_CHECK_CONNECTION_ON_STARTUP", "false");
        std::env::set_var("EXO_POSTGRES_URL", "postgres://a@localhost:0");
    }

    let model_system = builder::build_system_from_str(exo_model, "index.exo".to_string())
        .await
        .unwrap();
    let system_resolver = Arc::new(
        create_system_resolver_from_serialized_bytes(model_system, create_static_loaders())
            .await
            .unwrap(),
    );

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
