use crate::payastest::dbutils::{createdb_psql, dropdb_psql, run_psql};
use crate::payastest::loader::ParsedTestfile;
use crate::payastest::loader::TestfileOperation;
use actix_web::client::Client;
use anyhow::{anyhow, bail, Context, Result};
use serde::Serialize;
use std::io::Read;
use std::process::Command;
use std::process::Stdio;

#[derive(Serialize)]
struct PayasPost {
    query: String,
    variables: serde_json::Value,
}

pub async fn run_testfile(testfile: &ParsedTestfile, dburl: String) -> Result<bool> {
    let mut test_counter: usize = 0;
    const PORT_BASE: usize = 34140;
    let mut success: bool = true;

    // iterate through our tests
    for (test_name, test_op) in &testfile.test_operations {
        test_counter += 1;

        let dbname = format!("{}test{}", &testfile.unique_dbname, &test_counter);

        // create a database
        dropdb_psql(&dbname, &dburl).ok(); // clear any existing databases
        let (dburl_for_payas, dbusername) = createdb_psql(&dbname, &dburl)?;

        // select a port
        // TODO: check that port is free
        let port = PORT_BASE + test_counter;
        let endpoint = format!("http://127.0.0.1:{}/", port);

        // create the schema
        println!(
            "#{} ({}) Initializing schema in {} ...",
            test_counter, test_name, dbname
        );
        let cli_child = Command::new("payas-cli")
            .arg(testfile.model_path.as_ref().unwrap())
            .output()?;

        if !cli_child.status.success() {
            bail!("Could not build schema.");
        }

        let query = std::str::from_utf8(&cli_child.stdout)?;
        println!("{}", &query);
        run_psql(query, &dburl_for_payas)?;

        // spawn a payas instance
        println!(
            "#{} ({}) Initializing payas-server ...",
            test_counter, test_name
        );
        let mut payas_child = Command::new("payas-server")
            .arg(testfile.model_path.as_ref().unwrap())
            .env("PAYAS_DATABASE_URL", dburl_for_payas)
            .env("PAYAS_DATABASE_USER", dbusername)
            .env("PAYAS_JWT_SECRET", "abc")
            .env("PAYAS_SERVER_PORT", port.to_string())
            .stdout(Stdio::piped())
            .spawn()
            .expect("payas-server failed to start");

        // wait for it to start
        const MAGIC_STRING: &str = "Started ";
        let mut server_stdout = payas_child.stdout.take().unwrap();
        let mut buffer = [0; MAGIC_STRING.len()];
        server_stdout.read_exact(&mut buffer)?; // block while waiting for process output
        let output = String::from(std::str::from_utf8(&buffer)?);

        if !output.eq(MAGIC_STRING) {
            bail!("Unexpected output from payas-server: {}", output)
        }

        // run the init section
        for operation in testfile.init_operations.iter() {
            println!("#{} ({}) Initializing database...", test_counter, test_name);
            run_operation(&endpoint, operation).await??;
        }

        // run test
        println!("#{} ({}) Testing ...", test_counter, test_name);
        let result = run_operation(&endpoint, test_op).await;

        // did the test run okay?
        match result {
            Ok(test_result) => {
                // check test results
                match test_result {
                    Ok(_) => {
                        println!("#{} ({}) OK\n", test_counter, test_name);
                    }

                    Err(e) => {
                        println!(
                            "#{} ({}) ASSERTION FAILED\n{:?}",
                            test_counter, test_name, e
                        );
                        success = false;
                    }
                }
            }
            Err(e) => {
                println!(
                    "#{} ({}) TEST EXECUTION FAILED\n{:?}",
                    test_counter, test_name, e
                );
                success = false;
            }
        }

        // kill payas
        payas_child.kill().ok();

        // drop the database
        dropdb_psql(&dbname, &dburl)?;
    }

    Ok(success)
}

type TestResult = Result<()>;

async fn run_operation(url: &str, gql: &TestfileOperation) -> Result<TestResult> {
    match gql {
        TestfileOperation::GqlDocument {
            document,
            variables,
            expected_payload,
        } => {
            let client = Client::default();
            let mut resp = client
                .post(url)
                .send_json(&PayasPost {
                    query: document.to_string(),
                    variables: variables.as_ref().unwrap().clone(),
                })
                .await
                .map_err(|e| anyhow!("Error sending POST request: {}", e))?;

            if !resp.status().is_success() {
                bail!(
                    "Bad response: {}",
                    resp.status().canonical_reason().unwrap()
                );
            }

            let json = resp
                .json()
                .await
                .context("Error parsing response into JSON")?;
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

        TestfileOperation::Sql(query, dburl) => {
            run_psql(query, dburl)?;
            Ok(Ok(()))
        }
    }
}
