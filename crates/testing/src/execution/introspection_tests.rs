// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use anyhow::{Result, anyhow};
use colored::Colorize;

use common::{
    http::{MemoryRequestHead, MemoryRequestPayload},
    operation_payload::OperationsPayload,
};
use core_plugin_shared::serializable_system::SerializableSystem;
use exo_deno::{Arg, error::DenoError};
use exo_env::MapEnvironment;
use serde_json::Value;
use std::{collections::HashMap, path::Path, sync::Arc};
use system_router::{
    SystemRouter, create_system_router_from_file, create_system_router_from_system,
};

use common::env_const::{
    EXO_CHECK_CONNECTION_ON_STARTUP, EXO_CONNECTION_POOL_SIZE, EXO_INTROSPECTION, EXO_POSTGRES_URL,
    EXO_UNSTABLE_ENABLE_RPC_API,
};

use super::{TestResult, TestResultKind};

use super::integration_test::run_query;

pub(super) async fn run_introspection_test(
    model_path: &Path,
    generate_rpc_expected: bool,
) -> Result<TestResult> {
    let log_prefix = format!("(introspection: {})\n :: ", model_path.display()).purple();
    println!("{log_prefix} Running introspection tests...");

    let router = {
        let static_loaders = server_common::create_static_loaders();

        let env = MapEnvironment::from([
            (EXO_POSTGRES_URL, "postgres://a@dummy-value"),
            (EXO_CONNECTION_POOL_SIZE, "1"),
            (EXO_INTROSPECTION, "true"),
            (EXO_CHECK_CONNECTION_ON_STARTUP, "false"),
            (EXO_UNSTABLE_ENABLE_RPC_API, "true"),
        ]);

        let exo_ir_file = format!("{}/target/index.exo_ir", model_path.display()).to_string();

        create_system_router_from_file(&exo_ir_file, static_loaders, Arc::new(env)).await?
    };

    let result = check_introspection(&router, model_path, generate_rpc_expected).await;

    match result {
        Ok(result) => match result {
            Ok(()) => Ok(TestResult {
                log_prefix: log_prefix.to_string(),
                result: TestResultKind::Success,
            }),

            Err(e) => Ok(TestResult {
                log_prefix: log_prefix.to_string(),
                result: TestResultKind::Fail(e),
            }),
        },
        Err(e) => Ok(TestResult {
            log_prefix: log_prefix.to_string(),
            result: TestResultKind::Fail(e),
        }),
    }
}

async fn create_introspection_request() -> Result<MemoryRequestPayload> {
    let query =
        introspection_util::execute_introspection_deno_function("introspectionQuery", vec![])
            .await?;

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
                        (EXO_UNSTABLE_ENABLE_RPC_API, "true"),
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

async fn check_introspection(
    system_router: &SystemRouter,
    model_path: &Path,
    generate_rpc_expected: bool,
) -> Result<Result<()>> {
    // Check GraphQL introspection
    if let Err(e) = check_graphql_introspection(system_router, model_path).await? {
        return Ok(Err(e));
    }

    // Check RPC introspection
    check_rpc_introspection(system_router, model_path, generate_rpc_expected).await
}

async fn check_graphql_introspection(
    system_router: &SystemRouter,
    model_path: &Path,
) -> Result<Result<()>> {
    let mut deno_module = introspection_util::create_introspection_deno_module().await?;

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
            let sdl = introspection_util::schema_sdl(introspection_result).await;

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
                            print_diff(&expected_sdl_path, &actual_sdl_path)?;
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
                                print_diff(&expected_sdl_path, &actual_sdl_path)?;
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

fn create_rpc_discover_request() -> MemoryRequestPayload {
    let request_head = MemoryRequestHead::new(
        HashMap::new(),
        HashMap::new(),
        http::Method::GET,
        "/rpc/discover".to_string(),
        Value::default(),
        None,
    );

    MemoryRequestPayload::new(Value::Null, request_head)
}

async fn check_rpc_introspection(
    system_router: &SystemRouter,
    model_path: &Path,
    generate_rpc_expected: bool,
) -> Result<Result<()>> {
    let schema_tests_dir_str = format!("{}/schema-tests", model_path.display());
    let schema_tests_dir = Path::new(&schema_tests_dir_str);
    let expected_path = schema_tests_dir.join("rpc-introspection.expected.json");

    // Only run RPC introspection tests if expected file exists or generation was requested
    // Temporarily (until the RPC support becomes a bit more stable) control for which tests to check schema
    if !(std::fs::exists(&expected_path)? || generate_rpc_expected) {
        // Skip RPC introspection test if expected file doesn't exist and generation not requested
        return Ok(Ok(()));
    }

    let request = create_rpc_discover_request();

    let rpc_introspection_result = run_query(request, system_router, &mut HashMap::new()).await?;

    // Validate internal consistency of the OpenRPC document
    validate_openrpc_refs(&rpc_introspection_result, model_path)?;

    // Pretty-print the JSON for easier diffing
    let rpc_json = serde_json::to_string_pretty(&rpc_introspection_result)?;

    std::fs::create_dir_all(schema_tests_dir)?;

    let actual_path = schema_tests_dir.join("rpc-introspection.actual.json");

    if !std::fs::exists(&expected_path)? {
        // Generate expected file only when generation was requested
        std::fs::write(&expected_path, &rpc_json)?;
    } else {
        let expected_json = std::fs::read_to_string(&expected_path)?;

        // Compare the JSONs line by line (to avoid Windows/Unix line ending issues)
        let json_line_count = rpc_json.lines().count();
        let expected_json_line_count = expected_json.lines().count();

        if json_line_count != expected_json_line_count {
            std::fs::write(&actual_path, &rpc_json)?;
            print_diff(&expected_path, &actual_path)?;
            return Err(anyhow!(
                "RPC OpenRPC schema does not match the expected schema in {}. Expected {} lines, got {} lines",
                model_path.display(),
                expected_json_line_count,
                json_line_count
            ));
        }

        let json_lines = rpc_json.lines();
        let expected_json_lines = expected_json.lines();

        for (line_number, (json_line, expected_json_line)) in
            json_lines.zip(expected_json_lines).enumerate()
        {
            if json_line.trim() != expected_json_line.trim() {
                std::fs::write(&actual_path, &rpc_json)?;
                print_diff(&expected_path, &actual_path)?;
                return Err(anyhow!(
                    "RPC OpenRPC schema does not match the expected schema in {}. Difference at line {}",
                    model_path.display(),
                    line_number + 1
                ));
            }
        }
    }

    // If the actual file exists (produced by the test in an earlier run), we should delete it
    if std::fs::exists(&actual_path)? {
        std::fs::remove_file(&actual_path)?;
    }

    Ok(Ok(()))
}

fn print_diff(expected_file: &Path, actual_file: &Path) -> Result<()> {
    let diff_output = std::process::Command::new("diff")
        .arg("-u")
        .arg("-b")
        .arg(expected_file)
        .arg(actual_file)
        .output()?;

    if diff_output.status.success() {
        Ok(())
    } else {
        // If the files are different, print the diff
        eprintln!("{}", String::from_utf8(diff_output.stdout)?);
        Err(anyhow!(
            "Files {} and {} differ",
            expected_file.display(),
            actual_file.display()
        ))
    }
}

/// Validate internal consistency of an OpenRPC document.
/// Ensures all $ref references point to existing schemas in components/schemas.
fn validate_openrpc_refs(doc: &Value, model_path: &Path) -> Result<()> {
    // Collect all defined schema names
    let defined_schemas: std::collections::HashSet<String> = doc
        .get("components")
        .and_then(|c| c.get("schemas"))
        .and_then(|s| s.as_object())
        .map(|schemas| schemas.keys().cloned().collect())
        .unwrap_or_default();

    // Collect all $ref values from the document
    let mut refs = Vec::new();
    collect_refs(doc, &mut refs);

    // Check each ref points to an existing schema
    let mut missing_refs = Vec::new();
    for ref_value in refs {
        // $ref format is "#/components/schemas/SchemaName"
        if let Some(schema_name) = ref_value.strip_prefix("#/components/schemas/")
            && !defined_schemas.contains(schema_name)
        {
            missing_refs.push(schema_name.to_string());
        }
    }

    if !missing_refs.is_empty() {
        // Deduplicate and sort for cleaner error message
        missing_refs.sort();
        missing_refs.dedup();
        return Err(anyhow!(
            "OpenRPC schema in {} has broken $ref references. Missing schemas: {}",
            model_path.display(),
            missing_refs.join(", ")
        ));
    }

    Ok(())
}

/// Recursively collect all $ref values from a JSON value.
fn collect_refs(value: &Value, refs: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            if let Some(Value::String(ref_value)) = map.get("$ref") {
                refs.push(ref_value.clone());
            }
            for v in map.values() {
                collect_refs(v, refs);
            }
        }
        Value::Array(arr) => {
            for v in arr {
                collect_refs(v, refs);
            }
        }
        _ => {}
    }
}
