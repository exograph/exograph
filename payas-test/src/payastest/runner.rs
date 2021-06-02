
use simple_error::SimpleError;
use crate::payastest::loader::ParsedTestfile;
use crate::payastest::dbutils::{createdb_psql, dropdb_psql, run_psql};
use std::error::Error;
use simple_error::bail;
use crate::payastest::loader::TestfileOperation;
use actix_web::client::Client;
use serde::Serialize;
use std::process::Command;
use std::{thread, time};

#[derive(Serialize)]
struct PayasPost {
    query: String,
    variables: serde_json::Value
}

pub async fn run_testfile(testfile: &ParsedTestfile, dburl: String) -> Result<bool, Box<dyn Error>> {
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
        println!("#{} Initializing schema in {} ...", test_counter, dbname);
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
        println!("#{} Initializing payas-server ...", test_counter);
        let mut payas_child = Command::new("payas-server")
            .arg(testfile.model_path.as_ref().unwrap())
            .env("PAYAS_DATABASE_URL", dburl_for_payas)
            .env("PAYAS_DATABASE_USER", dbusername)
            .env("PAYAS_SERVER_PORT", port.to_string()) 
            .spawn()
            .expect("payas-server failed to start");

        thread::sleep_ms(500);

        // run the init section
        for operation in testfile.init_operations.iter() {
            println!("#{} Initializing database...", test_counter);
            run_operation(&endpoint, operation).await??;
        }
            
        // run test
        println!("#{} Testing ...", test_counter);
        let result = run_operation(&endpoint, test_op).await;

        // did the test run okay? 
        match result {
            Ok(test_result) => { 
                // check test results 
                match test_result {
                    Ok(_) => { 
                        println!("#{} OK\n", test_counter); 
                    },

                    Err(e) => {
                        println!("#{} ASSERTION FAILED\n{}", test_counter, e.to_string()); 
                        success = false; 
                    }
                }
            },
            Err(e) => { 
                println!("#{} TEST EXECUTION FAILED\n{}", test_counter, e.to_string()); 
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

type TestResult = Result<(), Box<dyn Error>>;

async fn run_operation(url: &str, gql: &TestfileOperation) -> Result<TestResult, Box<dyn Error>> {
    match gql {
        TestfileOperation::GqlDocument { document, variables, expected_payload } => { 
            let client = Client::default(); 
            let mut resp = client.post(url)
                .send_json(&PayasPost {
                    query: document.to_string(),                        
                    variables: variables.as_ref().unwrap().clone()
                })
                .await?;
                
            if !resp.status().is_success() {
                println!("BAD");
                bail!("Request failed with following: {}", resp.status().canonical_reason().unwrap());
            }
          
            let json = resp.json().await.unwrap();
            let body: serde_json::Value = json; 
          
            match expected_payload {
                Some(expected_payload) => {
                    // expected response detected - do an assertion
                    if body == *expected_payload {
                        Ok(Ok(()))
                    } else {
                        return Ok(Err(Box::new(SimpleError::new(format!(
                                "➔ Expected:\n{},\n\n➔ Got:\n{}",
                                serde_json::to_string_pretty(&expected_payload)?,
                                serde_json::to_string_pretty(&body)?,
                            )))))   
    
                    }
                },
               
                None => {                    
                    // don't need to check anything
                    Ok(Ok(()))
                }
            }
            
        },

        TestfileOperation::Sql(query, dburl) => {
            run_psql(query, dburl)?;
            Ok(Ok(()))
        }
    }
}