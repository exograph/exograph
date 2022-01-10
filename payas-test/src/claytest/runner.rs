use anyhow::{anyhow, bail, Context, Result};
use isahc::{HttpClient, ReadResponseExt, Request};
use jsonwebtoken::{encode, EncodingKey, Header};
use rand::{distributions::Alphanumeric, Rng};
use regex::Regex;
use serde::Serialize;
use serde_json::{json, Map, Value};

use std::{
    collections::HashMap,
    env,
    io::{BufRead, BufReader, Write},
    process::{Command, Stdio},
    sync::{Arc, Mutex},
    time::SystemTime,
};

use crate::claytest::dbutils::{createdb_psql, dropdb_psql, run_psql};
use crate::claytest::loader::{ParsedTestfile, TestfileOperation};
use crate::claytest::model::*;

use super::{
    assertion::{self, evaluate_using_deno},
    testvariable_bindings::resolve_testvariable,
};

#[derive(Serialize)]
struct ClayPost {
    query: String,
    variables: serde_json::Value,
}

pub(crate) fn run_testfile(
    testfile: &ParsedTestfile,
    bootstrap_dburl: String,
    dev_mode: bool,
) -> Result<TestOutput> {
    // iterate through our tests
    let mut ctx = TestfileContext::default();

    let log_prefix = ansi_term::Color::Purple.paint(format!("({})\n :: ", testfile.name()));

    let dbname = testfile.dbname(dev_mode);
    ctx.dbname = Some(dbname.clone());

    // generate a JWT secret
    let jwtsecret: String = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(30)
        .map(char::from)
        .collect();

    // create a database
    dropdb_psql(&dbname, &bootstrap_dburl).ok(); // clear any existing databases
    let (dburl_for_clay, dbusername) = createdb_psql(&dbname, &bootstrap_dburl)?;
    ctx.dburl = Some(dburl_for_clay.clone());

    // create the schema
    println!("{} Initializing schema in {} ...", log_prefix, dbname);

    let cli_child = cmd("clay")
        .args(["schema", "create", &testfile.model_path_string()])
        .output()?;

    if !cli_child.status.success() {
        eprintln!("{}", std::str::from_utf8(&cli_child.stderr).unwrap());
        bail!("Could not build schema.");
    }

    let query = std::str::from_utf8(&cli_child.stdout)?;
    run_psql(query, &dburl_for_clay)?;

    // spawn a clay instance
    println!("{} Initializing clay-server ...", log_prefix);

    let check_on_startup = if rand::random() { "true" } else { "false" };

    let (cmd_name, args) = if dev_mode {
        (
            "clay",
            vec!["serve".to_string(), testfile.model_path_string()],
        )
    } else {
        ("clay-server", vec![testfile.model_path_string()])
    };

    ctx.server = Some(
        cmd(cmd_name)
            .args(args)
            .env("CLAY_DATABASE_URL", &dburl_for_clay)
            .env("CLAY_DATABASE_USER", dbusername)
            .env("CLAY_JWT_SECRET", &jwtsecret)
            .env("CLAY_CONNECTION_POOL_SIZE", "1") // Otherwise we get a "too many connections" error
            .env("CLAY_CHECK_CONNECTION_ON_STARTUP", check_on_startup) // Should have no effect so make it random
            .env("CLAY_SERVER_PORT", "0") // ask clay-server to select a free port
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("clay-server failed to start")?,
    );

    // wait for it to start
    const MAGIC_STRING: &str = "Started server on 0.0.0.0:";

    let mut server_stdout = BufReader::new(ctx.server.as_mut().unwrap().stdout.take().unwrap());
    let mut server_stderr = BufReader::new(ctx.server.as_mut().unwrap().stderr.take().unwrap());

    let mut line = String::new();
    server_stdout.read_line(&mut line).context(format!(
        r#"Failed to read output line for "{}" server"#,
        testfile.name()
    ))?;

    if !line.starts_with(MAGIC_STRING) {
        bail!(
            r#"Unexpected output from clay-server "{}", {}: {}"#,
            testfile.name(),
            dev_mode,
            line
        )
    }

    // take the digits part which represents the port (and ignore other information such as time to start the server)
    let port: String = line
        .trim_start_matches(MAGIC_STRING)
        .chars()
        .take_while(|c| c.is_digit(10))
        .collect();
    let endpoint = format!("http://127.0.0.1:{}/", port);

    // spawn threads to continually drain stdout and stderr
    let output_mutex = Arc::new(Mutex::new(String::new()));

    let stdout_output = output_mutex.clone();
    let _stdout_drain = std::thread::spawn(move || loop {
        let mut buf = String::new();
        let _ = server_stdout.read_line(&mut buf);
        stdout_output.lock().unwrap().push_str(&buf);
    });

    let stderr_output = output_mutex.clone();
    let _stderr_drain = std::thread::spawn(move || loop {
        let mut buf = String::new();
        let _ = server_stderr.read_line(&mut buf);
        stderr_output.lock().unwrap().push_str(&buf);
    });

    let mut testvariables = HashMap::new();

    // run the init section
    println!("{} Initializing database...", log_prefix);
    for operation in testfile.init_operations.iter() {
        let result = run_operation(
            &endpoint,
            operation,
            &jwtsecret,
            &dburl_for_clay,
            &testvariables,
        )
        .with_context(|| {
            format!(
                "While initializing database for testfile {}",
                testfile.name()
            )
        })?;

        match result {
            OperationResult::Finished { variables } => {
                testvariables.extend(variables);
            }

            OperationResult::AssertFailed(_) | OperationResult::AssertPassed { .. } => {
                panic!("did not expect assertions in setup")
            }
        }
    }

    // run test
    println!("{} Testing ...", log_prefix);
    let result = run_operation(
        &endpoint,
        testfile.test_operation.as_ref().unwrap(),
        &jwtsecret,
        &dburl_for_clay,
        &testvariables,
    );

    // did the test run okay?
    let success = match result {
        Ok(test_result) => {
            // check test results
            match test_result {
                OperationResult::AssertPassed {
                    variables: _, // TODO: we will extend with this set of variables when we do multi-stage tests (#314)
                } => TestResult::Success,
                OperationResult::AssertFailed(e) => TestResult::AssertionFail(e),

                OperationResult::Finished { .. } => {
                    panic!("did not specify assertion inside testfile operation")
                }
            }
        }
        Err(e) => TestResult::SetupFail(e),
    };

    let output: String = output_mutex.lock().unwrap().clone();

    Ok(TestOutput {
        log_prefix: log_prefix.to_string(),
        result: success,
        output,
        dev_mode,
    })
    // implicit ctx drop
}

fn cmd(binary_name: &str) -> Command {
    match env::var("CLAY_USE_CARGO") {
        Ok(cargo_env) if &cargo_env == "1" => {
            let mut cmd = Command::new("cargo");
            cmd.args(["run", "--bin", binary_name, "--"]);
            cmd
        }
        _ => Command::new(binary_name),
    }
}

enum OperationResult {
    Finished {
        variables: HashMap<String, serde_json::Value>,
    },

    AssertPassed {
        variables: HashMap<String, serde_json::Value>,
    },
    AssertFailed(anyhow::Error),
}

fn run_operation(
    url: &str,
    gql: &TestfileOperation,
    jwtsecret: &str,
    dburl: &str,
    testvariables: &HashMap<String, serde_json::Value>,
) -> Result<OperationResult> {
    match gql {
        TestfileOperation::GqlDocument {
            document,
            testvariable_bindings,
            variables,
            expected_payload,
            auth,
        } => {
            let mut req = Request::post(url);

            // process substitutions in query variables section
            let variables = variables
                .as_ref()
                .map(|vars| evaluate_using_deno(vars, testvariables))
                .transpose()?;

            // remove @bind directives from our query
            // TODO: could we take them out of ExecutableDocument and serialize that instead?
            let query = Regex::new(r"@bind\(.*\)")?
                .replace_all(document, "")
                .to_string();

            // add JWT token if specified
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
                    &EncodingKey::from_secret(jwtsecret.as_ref()),
                )
                .unwrap();
                req = req.header("Authorization", format!("Bearer {}", token));
            };

            // run the operation
            let req =
                req.header("Content-Type", "application/json")
                    .body(serde_json::to_string(&ClayPost {
                        query,
                        variables: variables
                            .as_ref()
                            .unwrap_or(&Value::Object(Map::new()))
                            .clone(),
                    })?)?;

            let client = HttpClient::new()?;
            let mut resp = client
                .send(req)
                .map_err(|e| anyhow!("Error sending POST request: {}", e))?;

            if !resp.status().is_success() {
                bail!(
                    "Bad response: {}",
                    resp.status().canonical_reason().unwrap()
                );
            }

            let json = resp.json().with_context(|| {
                format!("Error parsing response into JSON: {}", resp.text().unwrap())
            })?;
            let body: serde_json::Value = json;

            // resolve testvariables from the result of our current operation
            let resolved_variables_keys = testvariable_bindings.keys().cloned();
            let resolved_variables_values = testvariable_bindings
                .keys()
                .map(|name| resolve_testvariable(name, &body, testvariable_bindings))
                .collect::<Result<Vec<_>>>()?
                .into_iter();
            let resolved_variables: HashMap<_, _> = resolved_variables_keys
                .zip(resolved_variables_values)
                .collect();

            match expected_payload {
                Some(expected_payload) => {
                    // expected response specified - do an assertion

                    // provide the following inside $ object
                    // - query variables
                    // - testvariables specified to run_operation at the start
                    // - testvariables resolved just now from the result of our current operation
                    let variables = match variables.unwrap_or_else(|| Value::Object(Map::new())) {
                        Value::Object(map) => {
                            let mut variable_map = HashMap::new();
                            variable_map.extend(testvariables.clone());
                            variable_map.extend(map);
                            variable_map.extend(resolved_variables.clone());
                            variable_map
                        }

                        _ => panic!("variables is not an Object"),
                    };

                    match assertion::dynamic_assert_using_deno(expected_payload, body, &variables) {
                        Ok(()) => Ok(OperationResult::AssertPassed {
                            variables: resolved_variables,
                        }),
                        Err(e) => Ok(OperationResult::AssertFailed(e)),
                    }
                }

                None => {
                    // don't need to check anything

                    Ok(OperationResult::Finished {
                        variables: resolved_variables,
                    })
                }
            }
        }

        TestfileOperation::Sql(query) => {
            run_psql(query, dburl)?;
            Ok(OperationResult::Finished {
                variables: Default::default(),
            })
        }
    }
}

pub(crate) fn build_claypot_file(path: &str) -> Result<()> {
    let build_child = cmd("clay").args(["build", path]).output()?;

    if !build_child.status.success() {
        std::io::stdout().write_all(&build_child.stdout).unwrap();
        std::io::stderr().write_all(&build_child.stderr).unwrap();
        bail!("Could not build the claypot.");
    }
    Ok(())
}
