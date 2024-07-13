// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use anyhow::{anyhow, bail, Context, Result};
use colored::Colorize;

use common::env_const::{
    EXO_CHECK_CONNECTION_ON_STARTUP, EXO_CONNECTION_POOL_SIZE, EXO_INTROSPECTION, EXO_JWT_SECRET,
    EXO_POSTGRES_URL,
};
use core_plugin_interface::trusted_documents::TrustedDocumentEnforcement;
use core_resolver::http::RequestHead;
use core_resolver::http::RequestPayload;
use core_resolver::system_resolver::{SystemResolutionError, SystemResolver};
use core_resolver::OperationsPayload;
use exo_sql::testing::db::EphemeralDatabaseServer;
use futures::future::OptionFuture;
use futures::FutureExt;
use jsonwebtoken::{encode, EncodingKey, Header};
use rand::{distributions::Alphanumeric, Rng};
use regex::Regex;
use resolver::{create_system_resolver, resolve_in_memory};
use serde_json::{json, Map, Value};
use unescape::unescape;

use std::net::{IpAddr, Ipv4Addr};
use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::time::Duration;
use std::{collections::HashMap, time::SystemTime};

use exo_env::MapEnvironment;

use crate::model::{resolve_testvariable, IntegrationTest, IntegrationTestOperation};

use super::assertion::{dynamic_assert_using_deno, evaluate_using_deno};
use super::{TestResult, TestResultKind};

/// Structure to hold open resources associated with a running testfile.
/// When dropped, we will clean them up.
struct TestfileContext {
    server: SystemResolver,
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
        let log_prefix = format!("({})\n :: ", self.name()).purple();

        let db_instance_name = format!("exotest_{:x}", md5::compute(self.name()));

        // create a database
        let db_instance = ephemeral_database.create_database(&db_instance_name)?;

        // iterate through our tests
        let mut ctx = {
            // generate a JWT secret
            let jwtsecret: String = rand::thread_rng()
                .sample_iter(&Alphanumeric)
                .take(30)
                .map(char::from)
                .collect();

            // create the schema
            println!("{log_prefix} Initializing schema for {} ...", self.name());

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
                bail!("Could not build schema for {}", self.name());
            }

            // Verify the schema to exercise the verification logic (which in-turn exercises the database introspection logic)
            let verify_child = cmd("exo")
                .args(["schema", "verify", "--database", &db_instance.url()])
                .current_dir(project_dir)
                .output()?;

            if !verify_child.status.success() {
                eprintln!("{}", std::str::from_utf8(&verify_child.stderr).unwrap());
                bail!("Could not verify schema for {}", self.name());
            }

            // spawn a exo instance
            println!("{log_prefix} Initializing exo-server ...");

            let telemetry_on = std::env::vars().any(|(name, _)| name.starts_with("OTEL_"));
            let mut extra_envs = self.extra_envs.clone();

            if telemetry_on {
                extra_envs.insert("OTEL_SERVICE_NAME".to_string(), self.name());
            }

            let server = {
                let static_loaders = server_common::create_static_loaders();

                let exo_ir_file = self.exo_ir_file_path(project_dir).display().to_string();

                let mut env = HashMap::from([
                    (
                        EXO_POSTGRES_URL.to_string(),
                        // set a common timezone for tests for consistency "-c TimeZone=UTC+00"
                        format!("{}?options=-c%20TimeZone%3DUTC%2B00", db_instance.url()),
                    ),
                    (EXO_JWT_SECRET.to_string(), jwtsecret.to_string()),
                    (EXO_CONNECTION_POOL_SIZE.to_string(), "1".to_string()),
                    (
                        EXO_CHECK_CONNECTION_ON_STARTUP.to_string(),
                        "false".to_string(),
                    ),
                    (EXO_INTROSPECTION.to_string(), "enabled".to_string()),
                ]);

                env.extend(extra_envs);

                let env = MapEnvironment::from(env);

                create_system_resolver(&exo_ir_file, static_loaders, Box::new(env)).await?
            };

            TestfileContext {
                server,
                jwtsecret,
                cookies: HashMap::new(),
                testvariables: HashMap::new(),
            }
        };

        // run the init section
        println!("{log_prefix} Initializing database...");
        for operation in self.init_operations.iter() {
            let result = run_operation(operation, &mut ctx).await.with_context(|| {
                format!("While initializing database for testfile {}", self.name())
            })?;

            match result {
                OperationResult::Finished => {}
                OperationResult::AssertFailed(_) | OperationResult::AssertPassed => {
                    panic!("did not expect assertions in setup")
                }
            }
        }

        // run test
        println!("{log_prefix} Testing ...");

        let mut fail = None;
        for operation in self.test_operations.iter() {
            let result = run_operation(operation, &mut ctx)
                .await
                .with_context(|| anyhow!("While running tests for {}", self.name()));

            match result {
                Ok(op_result) => match op_result {
                    OperationResult::AssertPassed | OperationResult::Finished => {}
                    OperationResult::AssertFailed(e) => {
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

enum OperationResult {
    Finished,
    AssertPassed,
    AssertFailed(anyhow::Error),
}

pub struct MemoryRequestHead {
    headers: HashMap<String, Vec<String>>,
    cookies: HashMap<String, String>,
    method: http::Method,
    path: String,
    query: Option<Value>,
}

impl MemoryRequestHead {
    pub fn new(
        cookies: HashMap<String, String>,
        method: http::Method,
        path: String,
        query: Option<Value>,
    ) -> Self {
        Self {
            headers: HashMap::new(),
            cookies,
            method,
            path,
            query,
        }
    }

    fn add_header(&mut self, key: &str, value: &str) {
        self.headers
            .entry(key.to_string().to_ascii_lowercase())
            .or_default()
            .push(value.to_string());
    }
}

impl RequestHead for MemoryRequestHead {
    fn get_headers(&self, key: &str) -> Vec<String> {
        if key.to_ascii_lowercase() == "cookie" {
            return self
                .cookies
                .iter()
                .map(|(k, v)| format!("{k}={v}"))
                .collect();
        } else {
            self.headers
                .get(&key.to_ascii_lowercase())
                .unwrap_or(&vec![])
                .clone()
        }
    }

    fn get_ip(&self) -> Option<std::net::IpAddr> {
        Some(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)))
    }

    fn get_method(&self) -> &http::Method {
        &self.method
    }

    fn get_path(&self) -> &str {
        &self.path
    }

    fn get_query(&self) -> Option<serde_json::Value> {
        self.query.clone()
    }
}

pub(super) struct MemoryRequestPayload {
    body: Value,
    head: MemoryRequestHead,
}

impl MemoryRequestPayload {
    pub(super) fn new(body: Value, head: MemoryRequestHead) -> Self {
        Self { body, head }
    }
}

impl RequestPayload for MemoryRequestPayload {
    fn take_body(&mut self) -> Value {
        self.body.take()
    }

    fn get_head(&self) -> &(dyn RequestHead + Send + Sync) {
        &self.head
    }
}

async fn run_operation(
    gql: &IntegrationTestOperation,
    ctx: &mut TestfileContext,
) -> Result<OperationResult> {
    let IntegrationTestOperation {
        document,
        operations_metadata,
        variables,
        expected_payload,
        auth,
        headers,
        deno_prelude,
    } = gql;

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
        ctx.cookies.clone(),
        http::Method::POST,
        "/graphql".to_string(),
        None,
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
    let body = run_query(request, &ctx.server, &mut ctx.cookies).await;

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

    match expected_payload {
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
                Ok(()) => Ok(OperationResult::AssertPassed),
                Err(e) => Ok(OperationResult::AssertFailed(e)),
            }
        }

        None => {
            // don't need to check anything

            Ok(OperationResult::Finished)
        }
    }
}

pub async fn run_query(
    request: impl RequestPayload,
    server: &SystemResolver,
    cookies: &mut HashMap<String, String>,
) -> Value {
    let res = resolve_in_memory(request, server, TrustedDocumentEnforcement::DoNotEnforce).await;

    match res {
        Ok(res) => {
            res.iter().for_each(|(_, r)| {
                r.headers.iter().for_each(|(k, v)| {
                    if k.to_ascii_lowercase() == "set-cookie" {
                        let cookie = v.split(';').next().unwrap();
                        let mut cookie = cookie.split('=');
                        let key = cookie.next().unwrap();
                        let value = cookie.next().unwrap();
                        cookies.insert(key.to_string(), value.to_string());
                    }
                });
            });

            serde_json::json!({
                "data": res.iter().map(|(name, result)| {
                    (name.clone(), result.body.to_json().unwrap())
                }).collect::<HashMap<String, Value>>(),
            })
        }
        Err(err) => {
            let mut out = serde_json::json!({
                "errors": [{
                    "message": unescape(&err.user_error_message()).unwrap()
                }]
            });

            if let SystemResolutionError::Validation(err) = err {
                let locations = err
                    .positions()
                    .iter()
                    .map(|e| {
                        serde_json::json!({
                            "line": e.line,
                            "column": e.column,
                        })
                    })
                    .collect::<Vec<_>>();

                out["errors"][0]["locations"] = locations.into();
            }

            out
        }
    }
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
