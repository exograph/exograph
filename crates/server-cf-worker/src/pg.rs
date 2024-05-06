use exo_sql::DatabaseClient;
use std::str::FromStr;
use wasm_bindgen::JsValue;

use worker::{postgres_tls::PassthroughTls, SecureTransport, Socket};

use deadpool_postgres::tokio_postgres::Config;

pub(super) struct WorkerPostgresConnect {
    config: Config,
    host: String,
    port: u16,
}

unsafe impl Send for WorkerPostgresConnect {}
unsafe impl Sync for WorkerPostgresConnect {}

impl WorkerPostgresConnect {
    pub(crate) async fn create_client(url: &str) -> Result<DatabaseClient, JsValue> {
        let config = Config::from_str(&url).map_err(|e| {
            JsValue::from_str(&format!(
                "Failed to parse PostgreSQL connection string: {:?}",
                e
            ))
        })?;
        let host = match &config.get_hosts()[0] {
            tokio_postgres::config::Host::Tcp(host) => Ok(host),
            // #[cfg(accessible(::tokio_postgres::config::Host::Unix))] (but can't: https://github.com/rust-lang/rust/issues/64797)
            #[allow(unreachable_patterns)]
            _ => Err(JsValue::from_str("Unix domain sockets are not supported")),
        }?
        .clone();

        let port = config.get_ports()[0];

        let connect = WorkerPostgresConnect { config, host, port };

        let client = DatabaseClient::from_connect(
            1,
            false,
            deadpool_postgres::tokio_postgres::Config::new(),
            connect,
            None,
            None,
        )
        .await
        .map_err(|e| JsValue::from_str(&format!("Error creating database client {:?}", e)))?;

        Ok(client)
    }
}

impl deadpool_postgres::Connect for WorkerPostgresConnect {
    fn connect(
        &self,
        _pg_config: &deadpool_postgres::tokio_postgres::Config,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<
                    Output = Result<
                        (
                            deadpool_postgres::tokio_postgres::Client,
                            tokio::task::JoinHandle<()>,
                        ),
                        deadpool_postgres::tokio_postgres::Error,
                    >,
                > + Send
                + '_,
        >,
    > {
        // We are misusing the timeout error here, but that is the only way to construct a
        // `tokio_postgres::Error` (which is required by the trait implementation). An alternative
        // would be `upwrap()`, which is decidedly worse, especially in WASM environments.
        Box::pin(async move {
            let socket = Socket::builder()
                .secure_transport(SecureTransport::StartTls)
                .connect(&self.host, self.port)
                .map_err(|e| {
                    tracing::error!("Error establishing connection to Postgres server: {:?}", e);
                    deadpool_postgres::tokio_postgres::Error::__private_api_timeout()
                })?;

            let (client, connection) = self.config.connect_raw(socket, PassthroughTls).await?;

            wasm_bindgen_futures::spawn_local({
                async move {
                    if let Err(error) = connection.await {
                        tracing::error!("connection error: {:?}", error);
                    }
                }
            });

            let tokio_runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .map_err(|e| {
                    tracing::error!("Error creating tokio runtime: {:?}", e);
                    deadpool_postgres::tokio_postgres::Error::__private_api_timeout()
                })?;
            let _guard = tokio_runtime.enter();

            Ok((client, tokio::spawn(async {})))
        })
    }
}
