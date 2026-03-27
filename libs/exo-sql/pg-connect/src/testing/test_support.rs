#![cfg(all(any(feature = "test-support", test), not(target_family = "wasm")))]

use std::future::Future;
use std::sync::LazyLock;
use std::sync::atomic::{AtomicBool, Ordering};

use tokio::sync::Mutex;

use crate::DatabaseClientManager;
use crate::TransactionMode;
use crate::connect::database_client::DatabaseClient;
use crate::testing::db::{
    EphemeralDatabaseLauncher, EphemeralDatabaseServer, generate_random_string,
};

/// This is used to ensure that we don't call cleanup if the database server is not initialized.
///
/// Implementation note: We won't need this once LazyLock::get() is stabilized.
static DATABASE_SERVER_INITIALIZED: AtomicBool = AtomicBool::new(false);

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
    if !DATABASE_SERVER_INITIALIZED.load(Ordering::Relaxed) {
        return;
    }

    let database_server = DATABASE_SERVER.blocking_lock();

    database_server.cleanup();
}

pub async fn with_client<Fut, T>(f: impl FnOnce(DatabaseClient) -> Fut) -> T
where
    Fut: Future<Output = T>,
{
    let database_name = generate_random_string();

    let database_server = DATABASE_SERVER.lock().await;
    let database_server = database_server.as_ref();

    DATABASE_SERVER_INITIALIZED.store(true, Ordering::Relaxed);

    let database = database_server.create_database(&database_name).unwrap();

    let client =
        DatabaseClientManager::from_url_direct(&database.url(), false, TransactionMode::ReadWrite)
            .await
            .unwrap()
            .get_client()
            .await
            .unwrap();

    f(client).await
}

/// Like `with_client`, but provides the database URL so the test can create multiple connections.
pub async fn with_db_url<Fut, T>(f: impl FnOnce(String) -> Fut) -> T
where
    Fut: Future<Output = T>,
{
    let database_name = generate_random_string();

    let database_server = DATABASE_SERVER.lock().await;
    let database_server = database_server.as_ref();

    DATABASE_SERVER_INITIALIZED.store(true, Ordering::Relaxed);

    let database = database_server.create_database(&database_name).unwrap();

    f(database.url()).await
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
