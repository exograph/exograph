use crate::payastest::dbutils::{createdb_psql, dropdb_psql, run_psql};
use crate::payastest::loader::ParsedTestfile;
use crate::payastest::loader::TestfileOperation;
use actix_web::client::Client;
use anyhow::{anyhow, bail, Context, Result};
use jsonwebtoken::{encode, EncodingKey, Header};
use port_scanner::request_open_port;
use rand::distributions::Alphanumeric;
use rand::Rng;
use serde::Serialize;
use serde_json::json;
use std::io::Read;
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

pub async fn run_testfile(testfile: &ParsedTestfile, dburl: String) -> Result<bool> {
    let mut test_counter: usize = 0;
    let mut success: bool = true;

    // iterate through our tests
    for (test_name, test_op) in &testfile.test_operations {
        let mut ctx = TestfileContext::default();
        test_counter += 1;

        let log_prefix = format!("({}/{})", testfile.name, test_name);
        let dbname = format!("{}_test_{}", &testfile.unique_dbname, &test_counter);
        ctx.dbname = Some(dbname.clone());

        // generate a JWT secret
        let jwtsecret: String = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(30)
            .map(char::from)
            .collect();

        // create a database
        dropdb_psql(&dbname, &dburl).ok(); // clear any existing databases
        let (dburl_for_payas, dbusername) = createdb_psql(&dbname, &dburl)?;
        ctx.dburl = Some(dburl_for_payas.clone());

        // select a free port
        let port = request_open_port().context("No open ports available.")?;
        let endpoint = format!("http://127.0.0.1:{}/", port);

        // create the schema
        println!("{} Initializing schema in {} ...", log_prefix, dbname);
        let cli_child = Command::new("payas-cli")
            .arg(testfile.model_path.as_ref().unwrap())
            .output()?;

        if !cli_child.status.success() {
            bail!("Could not build schema.");
        }

        let query = std::str::from_utf8(&cli_child.stdout)?;
        run_psql(query, &dburl_for_payas)?;

        // spawn a payas instance
        println!("{} Initializing payas-server ...", log_prefix);
        ctx.server = Some(
            Command::new("payas-server")
                .arg(testfile.model_path.as_ref().unwrap())
                .env("PAYAS_DATABASE_URL", dburl_for_payas)
                .env("PAYAS_DATABASE_USER", dbusername)
                .env("PAYAS_JWT_SECRET", &jwtsecret)
                .env("PAYAS_SERVER_PORT", port.to_string())
                .stdout(Stdio::piped())
                .spawn()
                .expect("payas-server failed to start"),
        );

        // wait for it to start
        const MAGIC_STRING: &str = "Started ";
        let mut server_stdout = ctx.server.as_mut().unwrap().stdout.take().unwrap();
        let mut buffer = [0; MAGIC_STRING.len()];

        server_stdout.read_exact(&mut buffer)?; // block while waiting for process output
        let output = String::from(std::str::from_utf8(&buffer)?);

        if !output.eq(MAGIC_STRING) {
            bail!("Unexpected output from payas-server: {}", output)
        }

        // run the init section
        for operation in testfile.init_operations.iter() {
            println!("{} Initializing database...", log_prefix);
            run_operation(&endpoint, operation, &jwtsecret)
                .await
                .context("Error while initializing database")??;
        }

        // run test
        println!("{} Testing ...", log_prefix);
        let result = run_operation(&endpoint, test_op, &jwtsecret).await;

        // did the test run okay?
        match result {
            Ok(test_result) => {
                // check test results
                match test_result {
                    Ok(_) => {
                        println!("{} OK\n", log_prefix);
                    }

                    Err(e) => {
                        println!("{} ASSERTION FAILED\n{:?}", log_prefix, e);
                        success = false;
                    }
                }
            }
            Err(e) => {
                println!("{} TEST EXECUTION FAILED\n{:?}", log_prefix, e);
                success = false;
            }
        }

        // implicit ctx drop
    }

    Ok(success)
}

type TestResult = Result<()>;

async fn run_operation(url: &str, gql: &TestfileOperation, jwtsecret: &str) -> Result<TestResult> {
    match gql {
        TestfileOperation::GqlDocument {
            document,
            variables,
            expected_payload,
            auth,
        } => {
            let client = Client::default();
            let mut req = client.post(url);

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

            let mut resp = req
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
