
#![feature(async_closure)]

use simple_error::SimpleError;
use crate::payastest::loader::ParsedTestfile;
use crate::payastest::dbutils::{createdb_psql, dropdb_psql};
use std::error::Error;
use simple_error::bail;
use crate::payastest::loader::TestfileOperation;
use actix_web::client::Client;
use serde::Serialize;

#[derive(Serialize)]
struct PayasPost {
    query: String,
    variables: serde_json::Value
}

// FIXME implement other operations 
pub async fn run_testfile(testfile: &ParsedTestfile, dburl: String) -> Result<bool, Box<dyn Error>> {
    let mut test_counter: usize = 0;
    let mut success: bool = true;

    // iterate through our tests
    for (test_name, test_op) in &testfile.test_operations {
        test_counter += 1;

        let dbname = format!("{}test{}", &testfile.unique_dbname, &test_counter);

        // create a database
        let dburl_for_payas = createdb_psql(&dbname, &dburl)?;

        // spawn a payas instance 

        // run the setup section
        for operation in testfile.setup_operations.iter() {
            // TODO actually run operation
            println!("#{} Setting up schema...", test_counter);
        }

        // run the init section
        for operation in testfile.init_operations.iter() {
            // TODO actually run operation
            println!("#{} Initializing database...", test_counter)
        }
            
        // run test
        println!("#{} Testing ...", test_counter);
        let result = payas_gql_run("http://127.0.0.1:9876/", test_op).await;

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

        // drop the database
        dropdb_psql(&dbname, &dburl)?;
    }
  
    Ok(success)
}

type TestResult = Result<(), Box<dyn Error>>;

async fn payas_gql_run(url: &str, gql: &TestfileOperation) -> Result<TestResult, Box<dyn Error>> {
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

        _ => { bail!("Supplied operation is not a GraphQL operation.") }
    }
}