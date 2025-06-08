use common::env_const::{DATABASE_URL, EXO_POSTGRES_URL};
use exo_env::Environment;
use exo_sql::DatabaseClientManager;

use std::str::FromStr;
use wasm_bindgen::JsValue;

use worker::{SecureTransport, Socket, postgres_tls::PassthroughTls};

use tokio_postgres::Config;

use crate::WorkerEnvironment;

const EXO_HYPERDRIVE_BINDING: &str = "EXO_HYPERDRIVE_BINDING";

pub(super) struct WorkerPostgresConnect {
    config: Config,
}

unsafe impl Send for WorkerPostgresConnect {}
unsafe impl Sync for WorkerPostgresConnect {}

impl WorkerPostgresConnect {
    pub(crate) async fn create_client(
        env: &WorkerEnvironment,
    ) -> Result<DatabaseClientManager, JsValue> {
        let hyperdrive_binding = env.get(EXO_HYPERDRIVE_BINDING);

        let config = match hyperdrive_binding {
            Some(hyperdrive_binding) => {
                let hyperdrive = env.hyperdrive(&hyperdrive_binding)?;

                tracing::info!("Connecting to Postgres with hyperdrive {hyperdrive_binding}");

                let mut config = hyperdrive
                    .connection_string()
                    .parse::<tokio_postgres::Config>()
                    .map_err(|e| {
                        worker::Error::RustError(format!("Failed to parse configuration: {:?}", e))
                    })?;

                let host = hyperdrive.host();
                let port = hyperdrive.port();

                config.host(&host);
                config.port(port);

                config
            }
            None => {
                let url = env
                    .get(EXO_POSTGRES_URL)
                    .or(env.get(DATABASE_URL))
                    .ok_or_else(|| {
                        JsValue::from_str(&format!("{EXO_POSTGRES_URL} or {DATABASE_URL} not set"))
                    })?;

                tracing::info!("Connecting to Postgres directly");

                let config = Config::from_str(&url).map_err(|e| {
                    JsValue::from_str(&format!(
                        "Failed to parse PostgreSQL connection string: {:?}",
                        e
                    ))
                })?;

                config
            }
        };

        let connect = WorkerPostgresConnect { config };

        let client = DatabaseClientManager::from_connect_direct(
            false,
            tokio_postgres::Config::new(),
            connect,
        )
        .await
        .map_err(|e| JsValue::from_str(&format!("Error creating database client {:?}", e)))?;

        Ok(client)
    }
}

impl exo_sql::Connect for WorkerPostgresConnect {
    fn connect(
        &self,
        _config: &tokio_postgres::Config,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<
                    Output = Result<
                        (tokio_postgres::Client, tokio::task::JoinHandle<()>),
                        tokio_postgres::Error,
                    >,
                > + Send
                + '_,
        >,
    > {
        // We are misusing the timeout error here, but that is the only way to construct a
        // `tokio_postgres::Error` (which is required by the trait implementation). An alternative
        // would be `upwrap()`, which is decidedly worse, especially in WASM environments.

        Box::pin(async move {
            let host = match &self.config.get_hosts()[0] {
                tokio_postgres::config::Host::Tcp(host) => Ok(host),
                // #[cfg(accessible(::tokio_postgres::config::Host::Unix))] (but can't: https://github.com/rust-lang/rust/issues/64797)
                #[allow(unreachable_patterns)]
                _ => {
                    tracing::error!("Connecting to a Unix socket is not supported");
                    Err(tokio_postgres::Error::__private_api_timeout())
                }
            }?
            .to_string();

            let port = self.config.get_ports()[0];

            let socket = Socket::builder()
                .secure_transport(SecureTransport::StartTls)
                .connect(&host, port)
                .map_err(|e| {
                    tracing::error!("Error establishing connection to Postgres server: {:?}", e);
                    tokio_postgres::Error::__private_api_timeout()
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
                    tokio_postgres::Error::__private_api_timeout()
                })?;
            let _guard = tokio_runtime.enter();

            Ok((client, tokio::spawn(async {})))
        })
    }
}
