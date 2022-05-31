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
use crate::claytest::model::{TestOutput, TestResult, TestfileContext};

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
) -> Result<TestOutput> {
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

        // create a database
        dropdb_psql(&dbname, &bootstrap_dburl).ok(); // clear any existing databases
        let (dburl_for_clay, dbusername) = createdb_psql(&dbname, &bootstrap_dburl)?;

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

        let mut server = cmd("clay-server")
            .args(vec![testfile.model_path_string()])
            .env("CLAY_DATABASE_URL", &dburl_for_clay)
            .env("CLAY_DATABASE_USER", dbusername)
            .env("CLAY_JWT_SECRET", &jwtsecret)
            .env("CLAY_CONNECTION_POOL_SIZE", "1") // Otherwise we get a "too many connections" error
            .env("CLAY_CHECK_CONNECTION_ON_STARTUP", check_on_startup) // Should have no effect so make it random
            .env("CLAY_SERVER_PORT", "0") // ask clay-server to select a free port
            .env("CLAY_INTROSPECTION", "true")
            .envs(testfile.extra_envs.iter()) // add extra envs specified in testfile
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("clay-server failed to start")?;

        // wait for it to start
        const MAGIC_STRING: &str = "Started server on 0.0.0.0:";

        let mut server_stdout = BufReader::new(server.stdout.take().unwrap());
        let mut server_stderr = BufReader::new(server.stderr.take().unwrap());

        let mut line = String::new();
        server_stdout.read_line(&mut line).context(format!(
            r#"Failed to read output line for "{}" server"#,
            testfile.name()
        ))?;

        if !line.starts_with(MAGIC_STRING) {
            bail!(
                r#"Unexpected output from clay-server "{}": {}"#,
                testfile.name(),
                line
            )
        }

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

        // spawn an HttpClient for requests to clay
        let client = HttpClient::builder()
            .cookies()
            .build()
            .context("While initializing HttpClient")?;

        // take the digits part which represents the port (and ignore other information such as time to start the server)
        let port: String = line
            .trim_start_matches(MAGIC_STRING)
            .chars()
            .take_while(|c| c.is_digit(10))
            .collect();
        let endpoint = format!("http://127.0.0.1:{}/", port);

        TestfileContext {
            dbname,
            dburl: dburl_for_clay,
            server,
            endpoint,
            jwtsecret,
            client,
            output_mutex,
            testvariables: HashMap::new(),
        }
    };

    // run the init section
    println!("{} Initializing database...", log_prefix);
    for operation in testfile.init_operations.iter() {
        let result = run_operation(operation, &mut ctx).with_context(|| {
            let output: String = ctx.output_mutex.lock().unwrap().clone();
            println!("{}", output);

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
    println!("{} Testing ...", log_prefix);

    let mut fail = None;
    for operation in testfile.test_operation_stages.iter() {
        let result = run_operation(operation, &mut ctx)
            .with_context(|| anyhow!("While running tests for {}", testfile.name()));

        match result {
            Ok(op_result) => match op_result {
                OperationResult::AssertPassed | OperationResult::Finished => {}
                OperationResult::AssertFailed(e) => {
                    fail = Some(TestResult::AssertionFail(e));
                    break;
                }
            },

            Err(e) => {
                fail = Some(TestResult::SetupFail(e));
                break;
            }
        };
    }

    let success = fail.unwrap_or(TestResult::Success);
    let output: String = ctx.output_mutex.lock().unwrap().clone();

    Ok(TestOutput {
        log_prefix: log_prefix.to_string(),
        result: success,
        output,
    })
    // implicit ctx drop
}

fn cmd(binary_name: &str) -> Command {
    // Pick up the current executable path and replace the file with the specified binary
    // This allows us to invoke `target/debug/clay test ...` or `target/release/clay test ...`
    // without updating the PATH env.
    // Thus, for the former invocation if the `binary_name` is `clay-server` the command will become
    // `<full-path-to>/target/debug/clay-server`
    let mut executable = env::current_exe().expect("Could not retrive the current executable");
    executable.set_file_name(binary_name);
    Command::new(
        executable
            .to_str()
            .expect("Could not convert executable path to a string"),
    )
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
        } => {
            let mut req = Request::post(&ctx.endpoint);

            // process substitutions in query variables section
            // and extend our collection with the results
            let variables_map: Map<String, Value> = variables
                .as_ref()
                .map(|vars| evaluate_using_deno(vars, &ctx.testvariables))
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
                req = req.header("Authorization", format!("Bearer {}", token));
            };

            // add extra headers from testfile
            let headers = headers
                .as_ref()
                .map(|headers| evaluate_using_deno(headers, &ctx.testvariables))
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
                    "Bad response: {}",
                    resp.status().canonical_reason().unwrap()
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
            run_psql(query, &ctx.dburl)?;
            Ok(OperationResult::Finished)
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
