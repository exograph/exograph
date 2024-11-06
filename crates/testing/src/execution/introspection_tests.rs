// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use anyhow::{anyhow, Result};
use colored::Colorize;

use core_plugin_interface::serializable_system::SerializableSystem;
use core_resolver::OperationsPayload;
use exo_deno::{
    deno_core::{url::Url, ModuleType},
    deno_error::DenoError,
    deno_executor_pool::{DenoScriptDefn, ResolvedModule},
    Arg, DenoModule, DenoModuleSharedState, UserCode,
};
use exo_env::MapEnvironment;
use include_dir::{include_dir, Dir};
use serde_json::Value;
use std::{collections::HashMap, path::Path, sync::Arc};
use system_router::{
    create_system_router_from_file, create_system_router_from_system, SystemRouter,
};

use common::env_const::{
    EXO_CHECK_CONNECTION_ON_STARTUP, EXO_CONNECTION_POOL_SIZE, EXO_INTROSPECTION, EXO_POSTGRES_URL,
};

use super::{integration_test::MemoryRequestPayload, TestResult, TestResultKind};

use super::integration_test::{run_query, MemoryRequestHead};

const INTROSPECTION_ASSERT_JS: &str = include_str!("introspection_tests.js");
const GRAPHQL_NODE_MODULE: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/node_modules/graphql");

pub(super) async fn run_introspection_test(model_path: &Path) -> Result<TestResult> {
    let log_prefix = format!("(introspection: {})\n :: ", model_path.display()).purple();
    println!("{log_prefix} Running introspection tests...");

    let exo_ir_file = format!("{}/target/index.exo_ir", model_path.display()).to_string();

    let router = {
        let static_loaders = server_common::create_static_loaders();

        let env = MapEnvironment::from([
            (EXO_POSTGRES_URL, "postgres://a@dummy-value"),
            (EXO_CONNECTION_POOL_SIZE, "1"),
            (EXO_INTROSPECTION, "true"),
            (EXO_CHECK_CONNECTION_ON_STARTUP, "false"),
        ]);

        create_system_router_from_file(&exo_ir_file, static_loaders, Arc::new(env)).await?
    };

    let result = check_introspection(&router).await?;

    match result {
        Ok(()) => Ok(TestResult {
            log_prefix: log_prefix.to_string(),
            result: TestResultKind::Success,
        }),

        Err(e) => Ok(TestResult {
            log_prefix: log_prefix.to_string(),
            result: TestResultKind::Fail(e),
        }),
    }
}

async fn create_introspection_deno_module() -> Result<DenoModule> {
    let script = INTROSPECTION_ASSERT_JS;

    DenoModule::new(
        UserCode::LoadFromMemory {
            path: "file://internal/introspection_tests.js".to_owned(),
            script: DenoScriptDefn {
                modules: vec![(
                    Url::parse("file://internal/introspection_tests.js").unwrap(),
                    ResolvedModule::Module(
                        script.into(),
                        ModuleType::JavaScript,
                        Url::parse("file://internal/introspection_tests.js").unwrap(),
                        false,
                    ),
                )]
                .into_iter()
                .collect(),
                npm_snapshot: None,
            },
        },
        "ExographTest",
        vec![],
        vec![],
        vec![],
        DenoModuleSharedState::default(),
        Some("Error"),
        Some(HashMap::from([(
            "graphql".to_string(),
            &GRAPHQL_NODE_MODULE,
        )])),
        Some(vec![(
            // TODO: move to a Rust-based solution
            // maybe juniper: https://github.com/graphql-rust/juniper/issues/217

            // We are currently importing the `graphql` NPM module used by graphiql and running it through Deno to perform schema validation
            // As it only depends on deno_core and deno_runtime, our integration of Deno does not include the NPM implementation provided through deno_cli
            // Therefore, we need to patch certain things in this module through extra_sources to get scripts to run in Deno

            // ReferenceError: process is not defined
            //    at embedded://graphql/jsutils/instanceOf.mjs:11:16
            "embedded://graphql/jsutils/instanceOf.mjs",
            GRAPHQL_NODE_MODULE
                .get_file("jsutils/instanceOf.mjs")
                .unwrap()
                .contents_utf8()
                .unwrap()
                .replace("process.env.NODE_ENV === 'production'", "false"),
        )]),
    )
    .await
}

async fn create_introspection_request() -> Result<MemoryRequestPayload> {
    let query = tokio::task::spawn_blocking({
        move || {
            tokio::runtime::Handle::current().block_on(async move {
                let mut deno_module = create_introspection_deno_module().await?;
                deno_module
                    .execute_function("introspectionQuery", vec![])
                    .await
                    .map_err(|e| anyhow!("Error getting introspection query: {:?}", e))
            })
        }
    })
    .await??;

    let request_head = MemoryRequestHead::new(
        HashMap::new(),
        http::Method::POST,
        "/graphql".to_string(),
        Default::default(),
    );

    let operations_payload = OperationsPayload {
        operation_name: None,
        query: if let Value::String(s) = query {
            Some(s)
        } else {
            panic!("expected string")
        },
        variables: None,
        query_hash: None,
    };

    Ok(MemoryRequestPayload::new(
        operations_payload.to_json()?,
        request_head,
    ))
}

// Needed for `exp graphql schema` command
// TODO: Find a better home for this and associated functions
pub async fn get_introspection_result(serialized_system: SerializableSystem) -> Result<Value> {
    let request = create_introspection_request()
        .await
        .map_err(|e| anyhow!("Error getting introspection result: {:?}", e))?;

    tokio::task::spawn_blocking({
        move || {
            tokio::runtime::Handle::current().block_on(async move {
                let router = {
                    let static_loaders = server_common::create_static_loaders();

                    let env = MapEnvironment::from([
                        (EXO_POSTGRES_URL, "postgres://a@dummy-value"),
                        (EXO_CONNECTION_POOL_SIZE, "1"),
                        (EXO_INTROSPECTION, "true"),
                        (EXO_CHECK_CONNECTION_ON_STARTUP, "false"),
                    ]);

                    let env = Arc::new(env);

                    create_system_router_from_system(serialized_system, static_loaders, env).await?
                };

                Ok::<_, anyhow::Error>(run_query(request, &router, &mut HashMap::new()).await)
            })
        }
    })
    .await?
}

async fn check_introspection(system_router: &SystemRouter) -> Result<Result<()>> {
    let mut deno_module = create_introspection_deno_module().await?;

    let request = create_introspection_request().await?;

    let result = run_query(request, system_router, &mut HashMap::new()).await;

    let result = deno_module
        .execute_function(
            "assertSchema",
            vec![Arg::Serde(Value::String(result.to_string()))],
        )
        .await;

    match result {
        Ok(_) => Ok(Ok(())),
        Err(e) => match e {
            DenoError::Explicit(e) => Ok(Err(anyhow!(e))),
            e => Err(anyhow!(e)),
        },
    }
}
