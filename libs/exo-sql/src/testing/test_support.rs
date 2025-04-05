#![cfg(all(any(feature = "test-support", test), not(target_family = "wasm")))]

use std::future::Future;
use std::sync::LazyLock;
use std::sync::Mutex;
use std::sync::PoisonError;

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

#[ctor::dtor]
fn cleanup() {
    let database_server = DATABASE_SERVER
        .lock()
        .unwrap_or_else(PoisonError::into_inner);

    database_server.cleanup();
}

// We need Mutex, whose value can be accessed from non-async code (the cleanup function above).
// Thus we can't use tokio::sync::Mutex here.
// TODO: Find a better way to handle this.
#[allow(clippy::await_holding_lock)]
pub async fn with_client<Fut, T>(f: impl FnOnce(DatabaseClient) -> Fut) -> T
where
    Fut: Future<Output = T>,
{
    let database_name = generate_random_string();

    let database_server = DATABASE_SERVER.lock().unwrap();
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

pub async fn with_init_script<Fut, T>(init_script: &str, f: impl FnOnce(DatabaseClient) -> Fut) -> T
where
    Fut: Future<Output = T>,
{
    with_client(|client| async move {
        client.batch_execute(init_script).await.unwrap();

        f(client).await
    })
    .await
}
