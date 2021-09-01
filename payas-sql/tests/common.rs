use anyhow::Result;
use payas_sql::sql::database::Database;
use postgres_openssl::MakeTlsConnector;
use r2d2::PooledConnection;
use r2d2_postgres::PostgresConnectionManager;

pub struct TestContext {
    db_name: String,
    pub client: PooledConnection<PostgresConnectionManager<MakeTlsConnector>>,
}

impl Drop for TestContext {
    fn drop(&mut self) {
        let query = format!("DROP DATABASE \"{}\"", self.db_name);
        let _ = self.client.execute(query.as_str(), &[]).unwrap();
    }
}

pub fn setup_client(test_name: &str) -> Result<TestContext> {
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
    let query = format!("CREATE DATABASE \"{}\"", &test_db_name);
    let _ = setup_client.execute(query.as_str(), &[]).unwrap();

    let db = Database::from_env_helper(
        1,
        test_db_url.clone(),
        test_user.clone(),
        test_password.clone(),
        Some(test_db_name.clone()),
    )?;
    let client = db.get_client()?;

    Ok(TestContext {
        db_name: test_db_name,
        client,
    })
}
