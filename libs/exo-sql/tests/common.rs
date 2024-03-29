// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

/*
use anyhow::Result;
use exo_sql::{
    spec::TableSpec,
    sql::{database::Database, PhysicalTable},
};

pub struct TestContext {
    db_name: String,
    setup_db: Database,
    pub test_db: Option<Database>,
}

impl Drop for TestContext {
    fn drop(&mut self) {
        // need to drop our test database connection first before dropping the database
        let mut db = None;
        std::mem::swap(&mut self.test_db, &mut db);
        std::mem::drop(db);

        let query = format!("DROP DATABASE \"{}\"", self.db_name);
        self.setup_db
            .get_client()
            .await
            .unwrap()
            .execute(query.as_str(), &[])
            .unwrap();
    }
}

impl TestContext {
    pub fn get_database(&mut self) -> &mut Database {
        self.test_db.as_mut().unwrap()
    }
}
*/
/*
/// Creates a testing context. This sets up contextual resources
/// needed for the test, like PostgreSQL databases. Takes a name for
/// the current test (should be unique!)
pub fn create_context(_test_name: &str) -> Result<TestContext> {
    let test_db_url = std::env::var("EXO_TEST_POSTGRES_URL")?;
    let test_user = std::env::var("EXO_TEST_POSTGRES_USER").ok();
    let test_password = std::env::var("EXO_TEST_POSTGRES_PASSWORD").ok();
    let test_db_name = format!("exo_integration_test_{}", test_name);

    let setup_db = Database::from_env_helper(
        1,
        true,
        test_db_url.clone(),
        test_user.clone(),
        test_password.clone(),
        Some("postgres".to_owned()),
    )?;
    let mut setup_client = setup_db.get_client()?;

    // create our database
    let query2 = format!("CREATE DATABASE \"{}\"", &test_db_name);
    setup_client.execute(query2.as_str(), &[]).unwrap();

    let db = Database::from_env_helper(
        5,
        true,
        test_db_url,
        test_user,
        test_password,
        Some(test_db_name.clone()),
    )?;

    Ok(TestContext {
        db_name: test_db_name,
        setup_db,
        test_db: Some(db),
    })
}

/// Creates a table using a textual SQL query and returns the
/// table schema as a PhysicalTable.
/// Note: `table_name` should match the table name used in `query`!
pub fn create_physical_table(db: &Database, table_name: &str, query: &str) -> PhysicalTable {
    let mut client = db.get_client().unwrap();

    // create table
    client.query(query, &[]).unwrap();

    // get definition back from database
    let table_spec = TableSpec::from_db(&mut client, table_name).unwrap();

    if !table_spec.issues.is_empty() {
        for issue in table_spec.issues.iter() {
            eprintln!("{}", issue)
        }

        panic!()
    }

    table_spec.value.into()
}
*/
