use anyhow::{anyhow, bail, Context, Result};
use isahc::{HttpClient, ReadResponseExt, Request};
use jsonwebtoken::{encode, EncodingKey, Header};
use rand::{distributions::Alphanumeric, Rng};
use regex::Regex;
use serde::Serialize;
use serde_json::{json, Map, Value};

use std::{
    collections::HashMap, ffi::OsStr, io::Write, path::Path, process::Command, time::SystemTime,
};

use crate::claytest::common::{TestResult, TestResultKind, TestfileContext};
use crate::claytest::dbutils::dropdb_psql;
use crate::claytest::loader::{ParsedTestfile, TestfileOperation};
use crate::claytest::{
    common::{cmd, spawn_clay_server},
    dbutils::{createdb_psql, run_psql},
};

use super::{
    assertion::{self, evaluate_using_deno},
    testvariable_bindings::resolve_testvariable,
};

#[derive(Serialize)]
struct ClayPost {
    query: String,
    variables: Map<String, Value>,
}

pub(crate) fn run_testfile(
    testfile: &ParsedTestfile,
    bootstrap_dburl: String,
) -> Result<TestResult> {
    let log_prefix = ansi_term::Color::Purple.paint(format!("({})\n :: ", testfile.name()));

    // iterate through our tests
    let mut ctx = {
        let dbname = testfile.dbname();

        // generate a JWT secret
        let jwtsecret: String = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(30)
            .map(char::from)
            .collect();

        // drop any previously-existing databases
        dropdb_psql(&dbname, &bootstrap_dburl).ok();

        // create a database
        let db = createdb_psql(&dbname, &bootstrap_dburl)?;

        // create the schema
        println!("{log_prefix} Initializing schema in {dbname} ...");

        let cli_child = cmd("clay")
            .args(["schema", "create", &testfile.model_path_string()])
            .output()?;

        if !cli_child.status.success() {
            eprintln!("{}", std::str::from_utf8(&cli_child.stderr).unwrap());
            bail!("Could not build schema.");
        }

        let query = std::str::from_utf8(&cli_child.stdout)?;
        run_psql(query, &db)?;

        // spawn a clay instance
        println!("{log_prefix} Initializing clay-server ...");

        // Should have no effect so make it random
        let check_on_startup = if rand::random() { "true" } else { "false" };

        let server = spawn_clay_server(
            &testfile.model_path,
            [
                ("CLAY_POSTGRES_URL", db.connection_string.as_str()),
                ("CLAY_POSTGRES_USER", &db.db_username),
                ("CLAY_JWT_SECRET", &jwtsecret),
                ("CLAY_CONNECTION_POOL_SIZE", "1"), // Otherwise we get a "too many connections" error
                ("CLAY_CHECK_CONNECTION_ON_STARTUP", check_on_startup),
                ("CLAY_SERVER_PORT", "0"), // ask clay-server to select a free port
                ("CLAY_INTROSPECTION", "true"),
            ]
            .into_iter()
            .chain(
                // add extra envs specified in testfile
                testfile
                    .extra_envs
                    .iter()
                    .map(|(x, y)| (x.as_str(), y.as_str())),
            ),
        )?;

        // spawn an HttpClient for requests to clay
        let client = HttpClient::builder()
            .cookies()
            .build()
            .context("While initializing HttpClient")?;

        TestfileContext {
            db,
            server,
            jwtsecret,
            client,
            testvariables: HashMap::new(),
        }
    };

    // run the init section
    println!("{log_prefix} Initializing database...");
    for operation in testfile.init_operations.iter() {
        let result = run_operation(operation, &mut ctx).with_context(|| {
            let output: String = ctx.server.output.lock().unwrap().clone();
            println!("{output}");

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
    let output: String = ctx.server.output.lock().unwrap().clone();

    Ok(TestResult {
        log_prefix: log_prefix.to_string(),
        result: success,
        output,
    })
    // implicit ctx drop
}

enum OperationResult {
    Finished,
    AssertPassed,
    AssertFailed(anyhow::Error),
}

fn run_operation(gql: &TestfileOperation, ctx: &mut TestfileContext) -> Result<OperationResult> {
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
            let mut req = Request::post(&ctx.server.endpoint);

            let deno_prelude = deno_prelude.clone().unwrap_or_default();

            // process substitutions in query variables section
            // and extend our collection with the results
            let variables_map: Map<String, Value> = variables
                .as_ref()
                .map(|vars| evaluate_using_deno(vars, &deno_prelude, &ctx.testvariables))
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
                req = req.header("Authorization", format!("Bearer {token}"));
            };

            // add extra headers from testfile
            let headers = headers
                .as_ref()
                .map(|headers| evaluate_using_deno(headers, &deno_prelude, &ctx.testvariables))
                .transpose()?;

            if let Some(Value::Object(map)) = headers {
                for (header, value) in map.iter() {
                    req = req.header(
                        header,
                        value.as_str().expect("expected string for header value"),
                    )
                }
            }

            // run the operation
            let req =
                req.header("Content-Type", "application/json")
                    .body(serde_json::to_string(&ClayPost {
                        query,
                        variables: variables_map,
                    })?)?;

            let mut resp = ctx
                .client
                .send(req)
                .map_err(|e| anyhow!("Error sending POST request: {}", e))?;

            if !resp.status().is_success() {
                bail!(
                    "Bad response: {}, {}",
                    resp.status().canonical_reason().unwrap(),
                    resp.text()?
                );
            }

            let json = resp.json().with_context(|| {
                format!("Error parsing response into JSON: {}", resp.text().unwrap())
            })?;
            let body: serde_json::Value = json;

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
                    ) {
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
            run_psql(query, &ctx.db)?;
            Ok(OperationResult::Finished)
        }
    }
}

// Run all scripts of the "build*.sh" form in the same directory as the model
fn build_prerequisites(directory: &Path) -> Result<()> {
    let mut build_files = vec![];

    for dir_entry in directory.read_dir()? {
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

pub(crate) fn build_claypot_file(path: &Path) -> Result<()> {
    build_prerequisites(path.parent().unwrap())?;

    // Use std::env::current_exe() so that we run the same "clay" that invoked us (specifically, avoid using another clay on $PATH)
    run_command(
        std::env::current_exe()?.as_os_str().to_str().unwrap(),
        [OsStr::new("build"), path.as_os_str()],
        None,
        "Could not build the claypot.",
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
