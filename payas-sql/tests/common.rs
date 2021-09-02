use anyhow::Result;
use payas_sql::sql::database::Database;
use postgres_openssl::MakeTlsConnector;
use r2d2::PooledConnection;
use r2d2_postgres::PostgresConnectionManager;

pub struct TestContext {
    db_name: String,
    setup_db: Database,
    pub db: Option<Database>,
}

impl Drop for TestContext {
    fn drop(&mut self) {
        // need to drop our test database connection first before dropping the database
        let mut db = None;
        std::mem::swap(&mut self.db, &mut db);
        std::mem::drop(db);

        let query = format!("DROP DATABASE \"{}\"", self.db_name);
        self.setup_db
            .get_client()
            .unwrap()
            .execute(query.as_str(), &[])
            .unwrap();
    }
}

impl TestContext {
    pub fn get_database(&mut self) -> &mut Database {
        self.db.as_mut().unwrap()
    }
}

pub fn create_database(test_name: &str) -> Result<TestContext> {
    let test_db_url = std::env::var("CLAY_TEST_DATABASE_URL")?;
    let test_user = std::env::var("CLAY_TEST_DATABASE_USER").ok();
    let test_password = std::env::var("CLAY_TEST_DATABASE_PASSWORD").ok();
    let test_db_name = format!("clay_integration_test_{}", test_name);

    let setup_db = Database::from_env_helper(
        1,
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
        1,
        test_db_url.clone(),
        test_user.clone(),
        test_password.clone(),
        Some(test_db_name.clone()),
    )?;

    Ok(TestContext {
        db_name: test_db_name,
        setup_db,
        db: Some(db),
    })
}
