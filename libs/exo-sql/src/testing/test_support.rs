#![cfg(all(any(feature = "test-support", test), not(target_family = "wasm")))]

use std::future::Future;
use std::sync::LazyLock;
use tokio::sync::Mutex;

use crate::sql::connect::database_client::DatabaseClient;
use crate::testing::db::{
    generate_random_string, EphemeralDatabaseLauncher, EphemeralDatabaseServer,
};
use crate::DatabaseClientManager;

static DATABASE_SERVER: LazyLock<Mutex<Box<dyn EphemeralDatabaseServer + Send + Sync>>> =
    LazyLock::new(|| {
        Mutex::new(
            EphemeralDatabaseLauncher::from_env()
                .create_server()
                .unwrap(),
        )
    });

pub async fn with_client<Fut, T>(f: impl FnOnce(DatabaseClient) -> Fut) -> T
where
    Fut: Future<Output = T>,
{
    let database_name = generate_random_string();

    let database_server = DATABASE_SERVER.lock().await;
    let database_server = database_server.as_ref();

    let database = database_server.create_database(&database_name).unwrap();

    let client = DatabaseClientManager::from_url(&database.url(), true, None)
        .await
        .unwrap()
        .get_client()
        .await
        .unwrap();

    f(client).await
}

pub async fn with_schema<Fut, T>(schema: &str, f: impl FnOnce(DatabaseClient) -> Fut) -> T
where
    Fut: Future<Output = T>,
{
    with_client(|client| async move {
        client.batch_execute(schema).await.unwrap();

        f(client).await
    })
    .await
}
