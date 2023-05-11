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

use core_resolver::context::{Request, RequestContext, LOCAL_JWT_SECRET};
use core_resolver::system_resolver::{SystemResolutionError, SystemResolver};
use core_resolver::OperationsPayload;
use exo_sql::testing::db::EphemeralDatabaseServer;
use exo_sql::{LOCAL_CONNECTION_POOL_SIZE, LOCAL_URL};
use futures::future::OptionFuture;
use jsonwebtoken::{encode, EncodingKey, Header};
use rand::{distributions::Alphanumeric, Rng};
use regex::Regex;
use resolver::{
    create_system_resolver, resolve_in_memory, LOCAL_ALLOW_INTROSPECTION, LOCAL_ENVIRONMENT,
};
use serde::Serialize;
use serde_json::{json, Map, Value};
use unescape::unescape;

use std::net::{IpAddr, Ipv4Addr};
use std::path::PathBuf;
use std::{
    collections::HashMap, ffi::OsStr, io::Write, path::Path, process::Command, time::SystemTime,
};

use crate::exotest::common::{TestResult, TestResultKind, TestfileContext};
use crate::exotest::loader::{ParsedTestfile, TestfileOperation};
use crate::exotest::{common::cmd, dbutils::run_psql};

use super::{
    assertion::{self, evaluate_using_deno},
    testvariable_bindings::resolve_testvariable,
};

#[derive(Serialize)]
struct ExoPost {
    query: String,
    variables: Map<String, Value>,
}

pub(crate) async fn run_testfile(
    testfile: &ParsedTestfile,
    project_dir: &PathBuf,
    ephemeral_database: &dyn EphemeralDatabaseServer,
) -> Result<TestResult> {
    let log_prefix = format!("({})\n :: ", testfile.name()).purple();

    // iterate through our tests
    let mut ctx = {
        // generate a JWT secret
        let jwtsecret: String = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(30)
            .map(char::from)
            .collect();

        let db_instance_name = format!("exotest_{:x}", md5::compute(testfile.name()));

        // create a database
        let db_instance = ephemeral_database.create_database(&db_instance_name)?;

        // create the schema
        println!(
            "{log_prefix} Initializing schema for {} ...",
            testfile.name()
        );

        let cli_child = cmd("exo")
            .args(["schema", "create"])
            .current_dir(project_dir)
            .output()?;

        if !cli_child.status.success() {
            eprintln!("{}", std::str::from_utf8(&cli_child.stderr).unwrap());
            bail!("Could not build schema.");
        }

        let query = std::str::from_utf8(&cli_child.stdout)?;
        run_psql(query, db_instance.as_ref()).await?;

        // spawn a exo instance
        println!("{log_prefix} Initializing exo-server ...");

        let telemetry_on = std::env::vars().any(|(name, _)| name.starts_with("OTEL_"));
        let mut extra_envs = testfile.extra_envs.clone();

        if telemetry_on {
            extra_envs.insert("OTEL_SERVICE_NAME".to_string(), testfile.name());
        }

        let server = {
            let static_loaders = server_common::create_static_loaders();

            let exo_ir_file = testfile.exo_ir_file_path(project_dir);
            LOCAL_URL.with(|url| {
                // set a common timezone for tests for consistency "-c TimeZone=UTC+00"
                url.borrow_mut().replace(format!(
                    "{}?options=-c%20TimeZone%3DUTC%2B00",
                    db_instance.url()
                ));

                LOCAL_CONNECTION_POOL_SIZE.with(|pool_size| {
                    // Otherwise we get a "too many connections" error
                    pool_size.borrow_mut().replace(1);

                    LOCAL_JWT_SECRET.with(|jwt| {
                        jwt.borrow_mut().replace(jwtsecret.clone());

                        LOCAL_ALLOW_INTROSPECTION.with(|allow| {
                            allow.borrow_mut().replace(true);

                            LOCAL_ENVIRONMENT.with(|env| {
                                env.borrow_mut().replace(extra_envs.clone());

                                create_system_resolver(
                                    &exo_ir_file.display().to_string(),
                                    static_loaders,
                                )
                            })
                        })
                    })
                })
            })?
        };

        TestfileContext {
            db: db_instance,
            server,
            jwtsecret,
            cookies: HashMap::new(),
            testvariables: HashMap::new(),
        }
    };

    // run the init section
    println!("{log_prefix} Initializing database...");
    for operation in testfile.init_operations.iter() {
        let result = run_operation(operation, &mut ctx).await.with_context(|| {
            format!(
                "While initializing database for testfile {}",
                testfile.name()
            )
        })?;

        match result {
            OperationResult::Finished => {}
            OperationResult::AssertFailed(_) | OperationResult::AssertPassed { .. } => {
                panic!("did not expect assertions in setup")
            }
        }
    }

    // run test
    println!("{log_prefix} Testing ...");

    let mut fail = None;
    for operation in testfile.test_operation_stages.iter() {
        let result = run_operation(operation, &mut ctx)
            .await
            .with_context(|| anyhow!("While running tests for {}", testfile.name()));

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

enum OperationResult {
    Finished,
    AssertPassed,
    AssertFailed(anyhow::Error),
}

pub struct MemoryRequest {
    headers: HashMap<String, Vec<String>>,
    cookies: HashMap<String, String>,
}

impl MemoryRequest {
    pub fn new(cookies: HashMap<String, String>) -> Self {
        Self {
            headers: HashMap::new(),
            cookies,
        }
    }

    fn add_header(&mut self, key: &str, value: &str) {
        self.headers
            .entry(key.to_string().to_ascii_lowercase())
            .or_default()
            .push(value.to_string());
    }
}

impl Request for MemoryRequest {
    fn get_headers(&self, key: &str) -> Vec<String> {
        if key.to_ascii_lowercase() == "cookie" {
            return self
                .cookies
                .iter()
                .map(|(k, v)| format!("{}={}", k, v))
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
}

async fn run_operation(
    gql: &TestfileOperation,
    ctx: &mut TestfileContext,
) -> Result<OperationResult> {
    match gql {
        TestfileOperation::GqlDocument {
            document,
            testvariable_bindings,
            variables,
            expected_payload,
            auth,
            headers,
            deno_prelude,
        } => {
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

            let mut request = MemoryRequest::new(ctx.cookies.clone());

            // add JWT token if specified in testfile
            if let Some(auth) = auth {
                let mut auth = auth.clone();
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
                request.add_header("Authorization", &format!("Bearer {token}"));
            };

            request.add_header("Content-Type", "application/json");

            // add extra headers from testfile
            let headers = OptionFuture::from(headers.as_ref().map(|headers| async {
                evaluate_using_deno(headers, &deno_prelude, &ctx.testvariables).await
            }))
            .await
            .transpose()?;

            if let Some(Value::Object(map)) = headers {
                for (header, value) in map.iter() {
                    request.add_header(
                        header,
                        value.as_str().expect("expected string for header value"),
                    );
                }
            }

            let request_context = RequestContext::new(&request, vec![], &ctx.server)?;
            let operations_payload = OperationsPayload {
                operation_name: None,
                query,
                variables: Some(variables_map),
            };

            // run the operation
            let body = run_query(
                operations_payload,
                request_context,
                &ctx.server,
                &mut ctx.cookies,
            )
            .await;

            // resolve testvariables from the result of our current operation
            // and extend our collection with them
            let resolved_variables_keys = testvariable_bindings.keys().cloned();
            let resolved_variables_values = testvariable_bindings
                .keys()
                .map(|name| resolve_testvariable(name, &body, testvariable_bindings))
                .collect::<Result<Vec<_>>>()?
                .into_iter();
            let resolved_variables: HashMap<_, _> = resolved_variables_keys
                .zip(resolved_variables_values)
                .collect();
            ctx.testvariables.extend(resolved_variables);

            match expected_payload {
                Some(expected_payload) => {
                    // expected response specified - do an assertion
                    match assertion::dynamic_assert_using_deno(
                        expected_payload,
                        body,
                        &deno_prelude,
                        &ctx.testvariables,
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

        TestfileOperation::Sql(query) => {
            run_psql(query, ctx.db.as_ref()).await?;
            Ok(OperationResult::Finished)
        }
    }
}

pub async fn run_query(
    operations_payload: OperationsPayload,
    request_context: RequestContext<'_>,
    server: &SystemResolver,
    cookies: &mut HashMap<String, String>,
) -> Value {
    let res = resolve_in_memory(operations_payload, server, request_context).await;

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

// Run all scripts of the "build*.sh" form in the same directory as the model
fn build_prerequisites(directory: &Path) -> Result<()> {
    let mut build_files = vec![];

    for dir_entry in directory.join("tests").read_dir()? {
        let dir_entry = dir_entry?;
        let path = dir_entry.path();

        if path.is_file() {
            let file_name = path.file_name().unwrap().to_str().unwrap();
            if file_name.starts_with("build") && path.extension().unwrap() == "sh" {
                build_files.push(path);
            }
        }
    }

    build_files.sort();

    for build_file in build_files {
        run_command(
            "sh",
            vec![build_file.to_str().unwrap()],
            None,
            &format!("Build script at {} failed to run", build_file.display()),
        )?
    }

    Ok(())
}

pub(crate) fn build_exo_ir_file(path: &Path) -> Result<()> {
    build_prerequisites(path)?;

    // Use std::env::current_exe() so that we run the same "exo" that invoked us (specifically, avoid using another exo on $PATH)
    run_command(
        std::env::current_exe()?.as_os_str().to_str().unwrap(),
        [OsStr::new("build")],
        Some(path),
        "Could not build the exo_ir.",
    )
}

// Helper to run a command and return an error if it fails
fn run_command<I, S>(
    program: &str,
    args: I,
    current_dir: Option<&Path>,
    failure_message: &str,
) -> Result<()>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut command = Command::new(program);
    command.args(args);
    if let Some(current_dir) = current_dir {
        command.current_dir(current_dir);
    }
    let build_child = command.output()?;

    if !build_child.status.success() {
        std::io::stdout().write_all(&build_child.stdout).unwrap();
        std::io::stderr().write_all(&build_child.stderr).unwrap();
        bail!(failure_message.to_string());
    }

    Ok(())
}
