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

use ::common::env_const::{EXO_CHECK_CONNECTION_ON_STARTUP, EXO_POSTGRES_URL};
use exo_env::MapEnvironment;
use router::SystemRouter;
use serde_json::Value;
use server_aws_lambda::resolve;
use server_common::create_static_loaders;

pub async fn test_query(json_input: Value, exo_model: &str, expected: Value) {
    let context = lambda_runtime::Context::default();
    let event = lambda_runtime::LambdaEvent::new(json_input, context);

    // HACK: some env vars need to be set to create a SystemContext
    let env = MapEnvironment::from([
        (EXO_POSTGRES_URL, "postgres://a@localhost:0"),
        (EXO_CHECK_CONNECTION_ON_STARTUP, "false"),
    ]);

    let model_system = builder::build_system_from_str(exo_model, "index.exo".to_string(), vec![])
        .await
        .unwrap();

    let system_router =
        SystemRouter::new_from_system(model_system, create_static_loaders(), Box::new(env))
            .await
            .unwrap();

    let result = resolve(event, Arc::new(system_router)).await.unwrap();

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
