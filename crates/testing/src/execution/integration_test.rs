// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use anyhow::{Context, Result, anyhow, bail};
use colored::Colorize;

use common::env_const::{
    EXO_CHECK_CONNECTION_ON_STARTUP, EXO_CONNECTION_POOL_SIZE, EXO_INTROSPECTION, EXO_JWT_SECRET,
    EXO_POSTGRES_READ_WRITE, EXO_POSTGRES_URL,
};
use common::http::{MemoryRequestHead, MemoryRequestPayload, RequestPayload, ResponseBodyError};
use common::operation_payload::OperationsPayload;
use common::router::{PlainRequestPayload, Router};
use exo_sql::DatabaseClientManager;
use exo_sql::testing::db::EphemeralDatabaseServer;
use futures::FutureExt;
use futures::future::OptionFuture;
use jsonwebtoken::{EncodingKey, Header, encode};
use rand::{Rng, distr::Alphanumeric};
use regex::Regex;
use serde_json::{Map, Value, json};
use system_router::{SystemRouter, create_system_router_from_file};

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::Sender;
use std::time::Duration;
use std::{collections::HashMap, time::SystemTime};

use exo_env::MapEnvironment;
use exo_sql::TransactionMode;

use crate::execution::assertion::assert_using_deno;
use crate::model::{
    ApiOperation, ApiOperationInvariant, DatabaseOperation, InitOperation, IntegrationTest,
    resolve_testvariable,
};

use super::assertion::{dynamic_assert_using_deno, evaluate_using_deno};
use super::{TestResult, TestResultKind};

/// Structure to hold open resources associated with a running testfile.
/// When dropped, we will clean them up.
struct TestfileContext {
    database_url: String,
    router: SystemRouter,
    jwtsecret: String,
    cookies: HashMap<String, String>,
    testvariables: HashMap<String, serde_json::Value>,
}

impl IntegrationTest {
    pub async fn run(
        &self,
        project_dir: &PathBuf,
        ephemeral_database: &dyn EphemeralDatabaseServer,
        tx: Sender<Result<TestResult>>,
    ) {
        let mut retries_left = self.retries;
        let mut pause = 1000;
        loop {
            let result =
                std::panic::AssertUnwindSafe(self.run_no_retry(project_dir, ephemeral_database))
                    .catch_unwind()
                    .await;

            if result.is_err() {
                // Don't retry after a panic
                retries_left = 0;
            }

            let result = result.unwrap_or_else(|_| {
                Err(anyhow::anyhow!(
                    "Panic during test run: {}",
                    project_dir.display()
                ))
            });
            let test_succeeded = result
                .as_ref()
                .map(|t| t.is_success())
                .unwrap_or_else(|_| false);

            if retries_left == 0 || test_succeeded {
                tx.send(result).map_err(|_| ()).unwrap();
                break;
            }
            println!("Test with configured retries failed. Waiting for {pause} ms before retrying");
            tokio::time::sleep(Duration::from_millis(pause)).await;
            pause *= 2;
            retries_left -= 1;
        }
    }

    async fn run_no_retry(
        &self,
        project_dir: &PathBuf,
        ephemeral_database: &dyn EphemeralDatabaseServer,
    ) -> Result<TestResult> {
        let test_name = self.name();
        let log_prefix = format!("({})\n :: ", test_name).purple();

        let db_instance_name = format!("exotest_{:x}", md5::compute(&test_name));

        // create a database
        let db_instance = ephemeral_database.create_database(&db_instance_name)?;

        // iterate through our tests
        let mut ctx = {
            // generate a JWT secret
            let jwtsecret: String = rand::rng()
                .sample_iter(&Alphanumeric)
                .take(30)
                .map(char::from)
                .collect();

            // create the schema
            println!("{log_prefix} Initializing schema for {} ...", test_name);

            let migrate_child = cmd("exo")
                .args([
                    "schema",
                    "migrate",
                    "--database",
                    &db_instance.url(),
                    "--apply-to-database",
                ])
                .current_dir(project_dir)
                .output()?;

            if !migrate_child.status.success() {
                eprintln!("{}", std::str::from_utf8(&migrate_child.stderr).unwrap());
                bail!("Could not build schema for {}", test_name);
            }

            // Verify the schema to exercise the verification logic (which in-turn exercises the database introspection logic)
            let verify_child = cmd("exo")
                .args(["schema", "verify", "--database", &db_instance.url()])
                .current_dir(project_dir)
                .output()?;

            if !verify_child.status.success() {
                eprintln!("{}", std::str::from_utf8(&verify_child.stderr).unwrap());
                bail!("Could not verify schema for {}", test_name);
            }

            // spawn a exo instance
            println!("{log_prefix} Initializing exo-server ...");

            let telemetry_on = std::env::vars().any(|(name, _)| name.starts_with("OTEL_"));
            let mut extra_envs = self.extra_envs.clone();

            if telemetry_on {
                extra_envs.insert("OTEL_SERVICE_NAME".to_string(), test_name.clone());
            }

            let router = {
                let static_loaders = server_common::create_static_loaders();

                let exo_ir_file = self.exo_ir_file_path(project_dir).display().to_string();

                let separator = match db_instance.url().contains("?") {
                    true => "&",
                    false => "?",
                };
                let mut env = HashMap::from([
                    (
                        EXO_POSTGRES_URL.to_string(),
                        // set a common timezone for tests for consistency "-c TimeZone=UTC+00"
                        format!(
                            "{}{}options=-c%20TimeZone%3DUTC%2B00",
                            db_instance.url(),
                            separator
                        ),
                    ),
                    (EXO_JWT_SECRET.to_string(), jwtsecret.to_string()),
                    (EXO_CONNECTION_POOL_SIZE.to_string(), "1".to_string()),
                    (
                        EXO_CHECK_CONNECTION_ON_STARTUP.to_string(),
                        "false".to_string(),
                    ),
                    (EXO_INTROSPECTION.to_string(), "enabled".to_string()),
                    (EXO_POSTGRES_READ_WRITE.to_string(), "true".to_string()),
                ]);

                env.extend(extra_envs);

                let env = MapEnvironment::from(env);

                create_system_router_from_file(&exo_ir_file, static_loaders, Arc::new(env)).await?
            };

            TestfileContext {
                database_url: db_instance.url(),
                router,
                jwtsecret,
                cookies: HashMap::new(),
                testvariables: HashMap::new(),
            }
        };

        // run the init section
        println!("{log_prefix} Initializing database...");
        for operation in self.init_operations.iter() {
            let result = run_init_operation(operation, &mut ctx)
                .await
                .with_context(|| {
                    format!("While initializing database for testfile {}", test_name)
                })?;

            match result {
                OperationResult::Pass => {}
                OperationResult::Fail(error) => {
                    Err(anyhow!("Initialization failed: {error}"))?;
                }
            }
        }

        // run test
        println!("{log_prefix} Testing ...");

        let mut fail = None;
        for operation in self.test_operations.iter() {
            let result = assert_api_operation(operation, &mut ctx)
                .await
                .with_context(|| anyhow!("While running tests for {}", test_name));

            match result {
                Ok(op_result) => match op_result {
                    OperationResult::Pass => {}
                    OperationResult::Fail(e) => {
                        fail = Some(TestResultKind::Fail(e));
                        break;
                    }
                },

                Err(e) => {
                    fail = Some(TestResultKind::SetupFail(e));
                    break;
                }
            };
        }

        let success = fail.unwrap_or(TestResultKind::Success);

        Ok(TestResult {
            log_prefix: log_prefix.to_string(),
            result: success,
        })
        // implicit ctx drop
    }
}

#[derive(Debug)]
enum OperationResult {
    Pass,
    Fail(anyhow::Error),
}

async fn run_init_operation(
    operation: &InitOperation,
    ctx: &mut TestfileContext,
) -> Result<OperationResult> {
    match operation {
        InitOperation::Database(operation) => run_database_operation(operation, ctx).await,
        InitOperation::Api(operation) => assert_api_operation(operation, ctx).await,
    }
}

async fn run_database_operation(
    operation: &DatabaseOperation,
    ctx: &mut TestfileContext,
) -> Result<OperationResult> {
    let db_url = ctx.database_url.clone();

    let client = DatabaseClientManager::from_url(&db_url, true, None, TransactionMode::ReadWrite)
        .await?
        .get_client()
        .await?;

    client.batch_execute(operation.sql.as_str()).await?;

    Ok(OperationResult::Pass)
}

async fn assert_api_operation(
    operation: &ApiOperation,
    ctx: &mut TestfileContext,
) -> Result<OperationResult> {
    let ApiOperation {
        metadata: operations_metadata,
        expected_response: expected_payload,
        deno_prelude,
        invariants,
        ..
    } = operation;

    let deno_prelude = deno_prelude.clone().unwrap_or_default();

    let pre_results = collect_invariants_results(invariants, ctx).await?;

    let body = execute_api_operation(operation, ctx).await?;

    let post_results = collect_invariants_results(invariants, ctx).await?;

    let invariant_result = match assert_invariant_results(
        pre_results,
        post_results,
        &operations_metadata.unordered_paths,
    )
    .await
    {
        Ok(()) => OperationResult::Pass,
        Err(e) => OperationResult::Fail(e),
    };

    // resolve testvariables from the result of our current operation
    // and extend our collection with them
    let resolved_variables_keys = operations_metadata.bindings.keys().cloned();
    let resolved_variables_values = operations_metadata
        .bindings
        .keys()
        .map(|name| resolve_testvariable(name, &body, &operations_metadata.bindings))
        .collect::<Result<Vec<_>>>()?
        .into_iter();
    let resolved_variables: HashMap<_, _> = resolved_variables_keys
        .zip(resolved_variables_values)
        .collect();
    ctx.testvariables.extend(resolved_variables);

    let test_result = match expected_payload {
        Some(expected_payload) => {
            // expected response specified - do an assertion
            match dynamic_assert_using_deno(
                expected_payload,
                body,
                &deno_prelude,
                &ctx.testvariables,
                &operations_metadata.unordered_paths,
            )
            .await
            {
                Ok(()) => OperationResult::Pass,
                Err(e) => OperationResult::Fail(e),
            }
        }

        None => {
            // No expected response specified - just check for errors
            match body.get("errors") {
                Some(_) => OperationResult::Fail(anyhow!(
                    "Unexpected error in response: {}",
                    serde_json::to_string_pretty(&body)?
                )),
                None => OperationResult::Pass,
            }
        }
    };

    match (test_result, invariant_result) {
        (OperationResult::Pass, OperationResult::Pass) => Ok(OperationResult::Pass),
        (OperationResult::Fail(e), OperationResult::Pass) => Ok(OperationResult::Fail(e)),
        (OperationResult::Pass, OperationResult::Fail(e)) => Ok(OperationResult::Fail(e)),
        (OperationResult::Fail(e), OperationResult::Fail(_)) => Ok(OperationResult::Fail(e)),
    }
}

async fn collect_invariants_results(
    invariants: &[ApiOperationInvariant],
    ctx: &mut TestfileContext,
) -> Result<Vec<Value>> {
    let mut invariant_results: Vec<Value> = vec![];

    for invariant in invariants {
        let result = execute_api_operation(&invariant.operation, ctx).await?;
        invariant_results.push(result);
    }

    Ok(invariant_results)
}

async fn assert_invariant_results(
    pre_results: Vec<Value>,
    post_results: Vec<Value>,
    unordered_paths: &HashSet<Vec<String>>,
) -> Result<()> {
    if pre_results.len() != post_results.len() {
        return Err(anyhow!(
            "Invariants failed to return the same number of results before and after the operation"
        ));
    }

    for (pre_result, post_result) in pre_results.into_iter().zip(post_results.into_iter()) {
        if pre_result.get("errors").is_some() || post_result.get("errors").is_some() {
            println!("pre_result: {:?}", pre_result);
            println!("post_result: {:?}", post_result);
            return Err(anyhow!("Invariants queries should not return errors"));
        }
        assert_using_deno(post_result, pre_result, unordered_paths).await?;
    }

    Ok(())
}

async fn execute_api_operation(
    operation: &ApiOperation,
    ctx: &mut TestfileContext,
) -> Result<Value> {
    let ApiOperation {
        document,
        variables,
        auth,
        headers,
        deno_prelude,
        ..
    } = operation;

    let deno_prelude = deno_prelude.clone().unwrap_or_default();

    // process substitutions in query variables section
    // and extend our collection with the results
    let variables_map: Map<String, Value> = OptionFuture::from(
        variables
            .as_ref()
            .map(|vars| evaluate_using_deno(vars, &deno_prelude, &ctx.testvariables)),
    )
    .await
    .transpose()?
    .unwrap_or_else(|| Value::Object(Map::new()))
    .as_object()
    .expect("evaluation to finish with a variable map")
    .clone();
    ctx.testvariables.extend(variables_map.clone());

    // remove @bind directives from our query
    // TODO: could we take them out of ExecutableDocument and serialize that instead?
    let query = Regex::new(r"@bind\(.*\)")?
        .replace_all(document, "")
        .to_string();
    // similarly, remove @unordered directives
    let query = query.replace("@unordered", "");

    let mut request_head = MemoryRequestHead::new(
        HashMap::new(),
        ctx.cookies.clone(),
        http::Method::POST,
        "/graphql".to_string(),
        Value::default(),
        Some("127.0.0.1".to_string()),
    );

    // add JWT token if specified in testfile
    if let Some(auth) = auth {
        let mut auth = evaluate_using_deno(auth, "", &ctx.testvariables).await?;
        let auth_ref = auth.as_object_mut().unwrap();
        let epoch_time = SystemTime::UNIX_EPOCH.elapsed().unwrap().as_secs();

        // populate token with expiry information
        auth_ref.insert("iat".to_string(), json!(epoch_time));
        auth_ref.insert("exp".to_string(), json!(epoch_time + 60 * 60));

        let token = encode(
            &Header::default(),
            &auth,
            &EncodingKey::from_secret(ctx.jwtsecret.as_ref()),
        )
        .unwrap();
        request_head.add_header("Authorization", &format!("Bearer {token}"));
    };

    request_head.add_header("Content-Type", "application/json");

    // add extra headers from testfile
    let headers = OptionFuture::from(headers.as_ref().map(|headers| async {
        evaluate_using_deno(headers, &deno_prelude, &ctx.testvariables).await
    }))
    .await
    .transpose()?;

    if let Some(Value::Object(map)) = headers {
        for (header, value) in map.iter() {
            request_head.add_header(
                header,
                value.as_str().expect("expected string for header value"),
            );
        }
    }

    let operations_payload = OperationsPayload {
        operation_name: None,
        query: Some(query),
        variables: Some(variables_map),
        query_hash: None,
    };

    let request = MemoryRequestPayload::new(operations_payload.to_json()?, request_head);
    // run the operation
    Ok(run_query(request, &ctx.router, &mut ctx.cookies).await?)
}

pub async fn run_query(
    request: impl RequestPayload + Send + Sync + 'static,
    router: &SystemRouter,
    cookies: &mut HashMap<String, String>,
) -> Result<Value, ResponseBodyError> {
    let res = router
        .route(&PlainRequestPayload::external(Box::new(request)))
        .await
        .unwrap();

    res.headers.into_iter().for_each(|(k, v)| {
        if k.eq_ignore_ascii_case("set-cookie") {
            let cookie = v.split(';').next().unwrap();
            let mut cookie = cookie.split('=');
            let key = cookie.next().unwrap();
            let value = cookie.next().unwrap();
            cookies.insert(key.to_string(), value.to_string());
        }
    });

    res.body.to_json().await
}

use std::process::Command;

pub(crate) fn cmd(binary_name: &str) -> Command {
    // Pick up the current executable path and replace the file with the specified binary
    // This allows us to invoke `target/debug/exo test ...` or `target/release/exo test ...`
    // without updating the PATH env.
    // Thus, for the former invocation if the `binary_name` is `exo-server` the command will become
    // `<full-path-to>/target/debug/exo-server`
    let mut executable =
        std::env::current_exe().expect("Could not retrieve the current executable");
    executable.set_file_name(binary_name);
    Command::new(
        executable
            .to_str()
            .expect("Could not convert executable path to a string"),
    )
}
