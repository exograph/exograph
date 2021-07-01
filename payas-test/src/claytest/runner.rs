use crate::claytest::dbutils::{createdb_psql, dropdb_psql, run_psql};
use crate::claytest::loader::ParsedTestfile;
use crate::claytest::loader::TestfileOperation;
use anyhow::{anyhow, bail, Context, Result};
use isahc::HttpClient;
use isahc::ReadResponseExt;
use isahc::Request;
use jsonwebtoken::{encode, EncodingKey, Header};
use rand::distributions::Alphanumeric;
use rand::Rng;
use serde::Serialize;
use serde_json::{json, Map, Value};
use std::env;
use std::io::Read;
use std::io::{BufRead, BufReader};
use std::process::Child;
use std::process::Command;
use std::process::Stdio;
use std::time::SystemTime;

#[derive(Serialize)]
struct PayasPost {
    query: String,
    variables: serde_json::Value,
}

/// Structure to hold open resources associated with a running testfile.
/// When dropped, we will clean them up.
#[derive(Default)]
struct TestfileContext {
    dbname: Option<String>,
    dburl: Option<String>,
    server: Option<Child>,
}

impl Drop for TestfileContext {
    fn drop(&mut self) {
        // kill the started server
        if let Some(server) = &mut self.server {
            if let e @ Err(_) = server.kill() {
                println!("Error killing server instance: {:?}", e)
            }
        }

        // drop the database
        if let Some(dburl) = &self.dburl {
            if let Some(dbname) = &self.dbname {
                if let e @ Err(_) = dropdb_psql(&dbname, &dburl) {
                    println!("Error dropping {} using {}: {:?}", dbname, dburl, e)
                }
            }
        }
    }
}

pub fn run_testfile(testfile: &ParsedTestfile, bootstrap_dburl: String) -> Result<usize> {
    let mut successful_tests: usize = 0;

    // iterate through our tests
    let mut ctx = TestfileContext::default();

    let log_prefix = ansi_term::Color::Purple.paint(format!("({})", testfile.name));
    let dbname = &testfile.unique_dbname;
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

    let cli_child = clay_cmd()
        .args(["schema", "create", testfile.model_path.as_ref().unwrap()])
        .output()?;

    if !cli_child.status.success() {
        bail!("Could not build schema.");
    }

    let query = std::str::from_utf8(&cli_child.stdout)?;
    run_psql(query, &dburl_for_clay)?;

    // spawn a clay instance
    println!("{} Initializing clay-server ...", log_prefix);

    ctx.server = Some(
        clay_cmd()
            .args(["serve", testfile.model_path.as_ref().unwrap()])
            .env("CLAY_DATABASE_URL", &dburl_for_clay)
            .env("CLAY_DATABASE_USER", dbusername)
            .env("CLAY_JWT_SECRET", &jwtsecret)
            .env("CLAY_SERVER_PORT", "0") // ask clay-server to select a free port
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .context("clay-server failed to start")?,
    );

    // wait for it to start
    const MAGIC_STRING: &str = "Started server on 0.0.0.0:";

    let mut server_stdout = BufReader::new(ctx.server.as_mut().unwrap().stdout.take().unwrap());

    let mut buffer = [0; MAGIC_STRING.len()];
    server_stdout.read_exact(&mut buffer)?; // block while waiting for process output
    let output = String::from(std::str::from_utf8(&buffer)?);

    //println!("{}", output);

    if !output.eq(MAGIC_STRING) {
        bail!("Unexpected output from clay-server: {}", output)
    }

    let mut buffer_port = String::new();
    server_stdout.read_line(&mut buffer_port)?; // read port clay-server is using
    buffer_port.pop(); // remove newline
    let endpoint = format!("http://127.0.0.1:{}/", buffer_port);

    // run the init section
    println!("{} Initializing database...", log_prefix);
    for operation in testfile.init_operations.iter() {
        run_operation(&endpoint, operation, &jwtsecret, &dburl_for_clay)??
    }

    // run test
    println!("{} Testing ...", log_prefix);
    let result = run_operation(
        &endpoint,
        &testfile.test_operation.as_ref().unwrap(),
        &jwtsecret,
        &dburl_for_clay,
    );

    // did the test run okay?
    match result {
        Ok(test_result) => {
            // check test results
            match test_result {
                Ok(_) => {
                    println!("{} {}\n", log_prefix, ansi_term::Color::Green.paint("OK"));
                    successful_tests += 1;
                }

                Err(e) => {
                    println!(
                        "{} {}\n{:?}",
                        log_prefix,
                        ansi_term::Color::Yellow.paint("ASSERTION FAILED"),
                        e
                    );
                }
            }
        }
        Err(e) => {
            println!(
                "{} {}\n{:?}",
                log_prefix,
                ansi_term::Color::Red.paint("TEST SETUP FAILED"),
                e
            );
        }
    }

    // implicit ctx drop

    Ok(successful_tests)
}

fn clay_cmd() -> Command {
    match env::var("CLAY_USE_CARGO") {
        Ok(cargo_env) if &cargo_env == "1" => {
            let mut cmd = Command::new("cargo");
            cmd.args(["run", "--bin", "clay", "--"]);
            cmd
        }
        _ => Command::new("clay"),
    }
}

type TestResult = Result<()>;

fn run_operation(
    url: &str,
    gql: &TestfileOperation,
    jwtsecret: &str,
    dburl: &str,
) -> Result<TestResult> {
    match gql {
        TestfileOperation::GqlDocument {
            document,
            variables,
            expected_payload,
            auth,
        } => {
            let mut req = Request::post(url);

            // add JWT token if specified
            if let Some(auth) = auth {
                let mut auth = auth.clone();
                let auth_ref = auth.as_object_mut().unwrap();
                let epoch_time = SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap()
                    .as_secs();

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

            let req =
                req.header("Content-Type", "application/json")
                    .body(serde_json::to_string(&PayasPost {
                        query: document.to_string(),
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

            let json = resp.json().context("Error parsing response into JSON")?;
            let body: serde_json::Value = json;

            match expected_payload {
                Some(expected_payload) => {
                    // expected response detected - do an assertion
                    if body == *expected_payload {
                        Ok(Ok(()))
                    } else {
                        return Ok(Err(anyhow!(format!(
                            "➔ Expected:\n{},\n\n➔ Got:\n{}",
                            serde_json::to_string_pretty(&expected_payload).unwrap(),
                            serde_json::to_string_pretty(&body).unwrap(),
                        ))));
                    }
                }

                None => {
                    // don't need to check anything
                    Ok(Ok(()))
                }
            }
        }

        TestfileOperation::Sql(query) => {
            run_psql(query, dburl)?;
            Ok(Ok(()))
        }
    }
}
