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

use common::{
    http::{MemoryRequestHead, MemoryRequestPayload},
    operation_payload::OperationsPayload,
};
use core_plugin_shared::serializable_system::SerializableSystem;
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

use super::{TestResult, TestResultKind};

use super::integration_test::run_query;

const INTROSPECTION_ASSERT_JS: &str = include_str!("introspection_tests.js");
const GRAPHQL_NODE_MODULE: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/node_modules/graphql");

pub(super) async fn run_introspection_test(model_path: &Path) -> Result<TestResult> {
    let log_prefix = format!("(introspection: {})\n :: ", model_path.display()).purple();
    println!("{log_prefix} Running introspection tests...");

    let router = {
        let static_loaders = server_common::create_static_loaders();

        let env = MapEnvironment::from([
            (EXO_POSTGRES_URL, "postgres://a@dummy-value"),
            (EXO_CONNECTION_POOL_SIZE, "1"),
            (EXO_INTROSPECTION, "true"),
            (EXO_CHECK_CONNECTION_ON_STARTUP, "false"),
        ]);

        let exo_ir_file = format!("{}/target/index.exo_ir", model_path.display()).to_string();

        create_system_router_from_file(&exo_ir_file, static_loaders, Arc::new(env)).await?
    };

    let result = check_introspection(&router, model_path).await?;

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
    let query = execute_deno_function("introspectionQuery", vec![]).await?;

    let request_head = MemoryRequestHead::new(
        HashMap::new(),
        HashMap::new(),
        http::Method::POST,
        "/graphql".to_string(),
        Value::default(),
        None,
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

                Ok(run_query(request, &router, &mut HashMap::new()).await?)
            })
        }
    })
    .await?
}

pub async fn schema_sdl(schema_response: Value) -> Result<String> {
    let sdl = execute_deno_function("schemaSDL", vec![Arg::Serde(schema_response)]).await?;

    if let Value::String(s) = sdl {
        Ok(s)
    } else {
        Err(anyhow!("expected string"))
    }
}

async fn check_introspection(
    system_router: &SystemRouter,
    model_path: &Path,
) -> Result<Result<()>> {
    let mut deno_module = create_introspection_deno_module().await?;

    let request = create_introspection_request().await?;

    let introspection_result = run_query(request, system_router, &mut HashMap::new()).await?;

    let assert_schema_result = deno_module
        .execute_function(
            "assertSchema",
            vec![Arg::Serde(Value::String(introspection_result.to_string()))],
        )
        .await;

    match assert_schema_result {
        Ok(_) => {
            // Make sure the SDL generation also works
            let sdl = schema_sdl(introspection_result).await;

            match sdl {
                Ok(sdl) => {
                    // We use a separate directory for schema tests (and not 'tests'), since in a few cases,
                    // we soft-link the tests directory to run the same tests with different index.exo files.
                    // In such situations, we may (correctly) get different SDLs for each index.exo file,
                    // and we don't want to mix them up.
                    let schema_tests_dir_str = format!("{}/schema-tests", model_path.display());
                    let schema_tests_dir = Path::new(&schema_tests_dir_str);

                    std::fs::create_dir_all(schema_tests_dir)?;

                    let expected_sdl_path = schema_tests_dir.join("introspection.expected.graphql");
                    let actual_sdl_path = schema_tests_dir.join("introspection.actual.graphql");

                    if !std::fs::exists(&expected_sdl_path)? {
                        // If the expected file does not exist (first time running the test), write the SDL to the file
                        std::fs::write(&expected_sdl_path, &sdl)?;
                    } else {
                        let expected_sdl = std::fs::read_to_string(&expected_sdl_path)?;

                        // Compare the SDLs line by line (to avoid Windows/Unix line ending issues)
                        let sdl_line_count = sdl.lines().count();
                        let expected_sdl_line_count = expected_sdl.lines().count();

                        if sdl_line_count != expected_sdl_line_count {
                            std::fs::write(&actual_sdl_path, &sdl)?;
                            return Err(anyhow!(
                                "SDL does not match the expected schema in {}. Expected {} lines, got {} lines",
                                model_path.display(),
                                expected_sdl_line_count,
                                sdl_line_count
                            ));
                        }

                        let sdl_lines = sdl.lines();
                        let expected_sdl_lines = expected_sdl.lines();

                        for (line_number, (sdl_line, expected_sdl_line)) in
                            sdl_lines.zip(expected_sdl_lines).enumerate()
                        {
                            if sdl_line.trim() != expected_sdl_line.trim() {
                                std::fs::write(&actual_sdl_path, &sdl)?;
                                return Err(anyhow!(
                                    "SDL does not match the expected schema in {}. Difference at line {}",
                                    model_path.display(),
                                    line_number + 1
                                ));
                            }
                        }
                    }

                    // If the actual file exists (produced by the test in an earlier run), we should delete it
                    if std::fs::exists(&actual_sdl_path)? {
                        std::fs::remove_file(&actual_sdl_path)?;
                    }

                    Ok(Ok(()))
                }
                Err(e) => Err(e.context("Error getting schema SDL")),
            }
        }
        Err(e) => match e {
            DenoError::Explicit(e) => Err(anyhow!(e)),
            e => Err(e.into()),
        },
    }
}

async fn execute_deno_function(function_name: &str, args: Vec<Arg>) -> Result<Value> {
    let function_name = function_name.to_string();

    tokio::task::spawn_blocking({
        move || {
            tokio::runtime::Handle::current().block_on(async move {
                let mut deno_module = create_introspection_deno_module().await?;
                deno_module
                    .execute_function(&function_name, args)
                    .await
                    .map_err(|e| anyhow!("Error executing function: {:?}", e))
            })
        }
    })
    .await?
}
