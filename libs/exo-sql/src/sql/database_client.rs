// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::cell::RefCell;

#[cfg(feature = "postgres-url")]
use std::{env, fs::File, io::BufReader};

#[cfg(feature = "postgres-url")]
use common::env_const::{
    EXO_CHECK_CONNECTION_ON_STARTUP, EXO_CONNECTION_POOL_SIZE, EXO_POSTGRES_PASSWORD,
    EXO_POSTGRES_URL, EXO_POSTGRES_USER,
};

#[cfg(feature = "deadpool")]
use deadpool_postgres::{Client, Manager, ManagerConfig, Pool, RecyclingMethod};
#[cfg(not(feature = "deadpool"))]
use tokio_postgres::Client;

#[cfg(feature = "tls")]
use rustls::{Certificate, RootCertStore};
#[cfg(feature = "tls")]
use rustls_native_certs::load_native_certs;

#[cfg(feature = "postgres-url")]
use tokio_postgres::config::SslMode;

use tokio_postgres::Config;

use crate::database_error::DatabaseError;

// we spawn many resolvers concurrently in integration tests
thread_local! {
    pub static LOCAL_URL: RefCell<Option<String>> = const { RefCell::new(None) };
    pub static LOCAL_CONNECTION_POOL_SIZE: RefCell<Option<usize>> = const { RefCell::new(None) };
    pub static LOCAL_CHECK_CONNECTION_ON_STARTUP: RefCell<Option<bool>> = const { RefCell::new(None) };
}

pub struct DatabaseClient {
    #[cfg(feature = "deadpool")]
    pool: Pool,
    #[cfg(not(feature = "deadpool"))]
    client: std::sync::Mutex<
        Box<
            dyn Fn() -> std::pin::Pin<
                    Box<dyn std::future::Future<Output = Result<Client, DatabaseError>> + Send>,
                > + Send,
        >,
    >,
}

#[cfg(feature = "postgres-url")]
struct SslConfig {
    mode: SslMode,
    root_cert_path: Option<String>,
}

impl<'a> DatabaseClient {
    // pool_size_override useful when we want to explicitly control the pool size (for example, to 1, when importing database schema)
    #[cfg(feature = "postgres-url")]
    pub async fn from_env(pool_size_override: Option<usize>) -> Result<Self, DatabaseError> {
        let url = LOCAL_URL
            .with(|f| f.borrow().clone())
            .or_else(|| env::var(EXO_POSTGRES_URL).ok())
            .ok_or(DatabaseError::Config(format!(
                "Env {EXO_POSTGRES_URL} must be provided"
            )))?;

        let user = env::var(EXO_POSTGRES_USER).ok();
        let password = env::var(EXO_POSTGRES_PASSWORD).ok();
        let pool_size = pool_size_override.unwrap_or_else(|| {
            LOCAL_CONNECTION_POOL_SIZE
                .with(|f| *f.borrow())
                .or_else(|| {
                    env::var(EXO_CONNECTION_POOL_SIZE)
                        .ok()
                        .map(|pool_str| pool_str.parse::<usize>().unwrap())
                })
                .unwrap_or(10)
        });

        let check_connection = LOCAL_CHECK_CONNECTION_ON_STARTUP
            .with(|f| *f.borrow())
            .or_else(|| {
                env::var(EXO_CHECK_CONNECTION_ON_STARTUP)
                    .ok()
                    .map(|check| check.parse::<bool>().expect("Should be true or false"))
            })
            .unwrap_or(true);

        Self::from_helper(pool_size, check_connection, &url, user, password).await
    }

    #[cfg(feature = "postgres-url")]
    pub async fn from_db_url(url: &str) -> Result<Self, DatabaseError> {
        Self::from_helper(1, true, url, None, None).await
    }

    #[cfg(all(not(feature = "deadpool"), not(feature = "tls")))]
    pub async fn from_socket<
        S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
    >(
        check_connection: bool,
        stream_creator: impl Fn() -> S + Send + 'static,
        user: Option<String>,
        password: Option<String>,
    ) -> Result<Self, DatabaseError> {
        let mut config = Config::default();

        if let Some(user) = &user {
            config.user(user);
        }
        if let Some(password) = &password {
            config.password(password);
        }

        let db = Self {
            client: std::sync::Mutex::new(Box::new(move || {
                let stream = stream_creator();
                let my_config_clone = config.clone();
                Box::pin(async move {
                    Ok(my_config_clone
                        .connect_raw(stream, tokio_postgres::NoTls)
                        .await?
                        .0)
                })
            })),
        };

        if check_connection {
            let _ = db.get_client().await?;
        }

        Ok(db)
    }

    #[cfg(feature = "postgres-url")]
    async fn from_helper(
        pool_size: usize,
        check_connection: bool,
        url: &str,
        user: Option<String>,
        password: Option<String>,
    ) -> Result<Self, DatabaseError> {
        use std::str::FromStr;

        let (url, ssl_config) = Self::create_ssl_config(url)?;

        let mut config = Config::from_str(&url).map_err(|e| {
            DatabaseError::Delegate(e)
                .with_context("Failed to parse PostgreSQL connection string".into())
        })?;

        if let Some(user) = &user {
            config.user(user);
        }
        if let Some(password) = &password {
            config.password(password);
        }

        #[cfg(feature = "deadpool")]
        let manager_config = ManagerConfig {
            recycling_method: RecyclingMethod::Fast,
        };

        #[cfg(feature = "deadpool")]
        let manager = match ssl_config {
            Some(SslConfig {
                mode,
                root_cert_path: cert_path,
            }) => {
                #[cfg(feature = "tls")]
                {
                    config.ssl_mode(mode);

                    let tls = {
                        let mut root_store = RootCertStore::empty();

                        // If the cert path is provided, use it. Otherwise, use the native certs.
                        match cert_path {
                            Some(cert_path) => {
                                let cert_file = File::open(&cert_path).map_err(|e| {
                                    DatabaseError::Config(format!(
                                        "Failed to open certificate file '{}': {}",
                                        cert_path, e
                                    ))
                                })?;
                                let mut buf = BufReader::new(cert_file);
                                rustls_pemfile::certs(&mut buf)
                                    .collect::<Result<Vec<_>, _>>()
                                    .map_err(|_| {
                                        DatabaseError::Config("Invalid certificate".into())
                                    })?
                                    .into_iter()
                                    .map(|cert| root_store.add(&Certificate(cert.to_vec())))
                                    .collect::<Result<Vec<_>, _>>()?;
                            }
                            None => {
                                for cert in load_native_certs()? {
                                    root_store.add(&Certificate(cert.to_vec()))?;
                                }
                            }
                        }

                        let config = rustls::ClientConfig::builder()
                            .with_safe_defaults()
                            .with_root_certificates(root_store)
                            .with_no_client_auth();
                        tokio_postgres_rustls::MakeRustlsConnect::new(config)
                    };

                    Manager::from_config(config, tls, manager_config)
                }

                #[cfg(not(feature = "tls"))]
                {
                    panic!("TLS support is not enabled")
                }
            }
            None => Manager::from_config(config, tokio_postgres::NoTls, manager_config),
        };

        #[cfg(feature = "deadpool")]
        let pool = Pool::builder(manager)
            .max_size(pool_size)
            .build()
            .expect("Failed to create DB pool");

        #[cfg(feature = "deadpool")]
        let db = Self { pool };

        #[cfg(not(feature = "deadpool"))]
        let db = Self {
            client: std::sync::Mutex::new(Box::new(move || {
                let config_clone = config.clone();
                Box::pin(async move { Ok(config_clone.connect(tokio_postgres::NoTls).await?.0) })
            })),
        };

        if check_connection {
            let _ = db.get_client().await?;
        }

        Ok(db)
    }

    pub async fn get_client(&self) -> Result<Client, DatabaseError> {
        #[cfg(feature = "deadpool")]
        {
            Ok(self.pool.get().await?)
        }

        #[cfg(not(feature = "deadpool"))]
        {
            let fut = {
                let guard = (self.client).lock().unwrap();
                guard()
            };
            Ok(fut.await?)
        }
    }

    #[cfg(feature = "postgres-url")]
    fn create_ssl_config(url: &str) -> Result<(String, Option<SslConfig>), DatabaseError> {
        let url = url::Url::parse(url)
            .map_err(|_| DatabaseError::Config("Invalid database URL".into()))?;

        let mut ssl_param_string: Option<String> = None;
        let mut ssl_mode_string: Option<String> = None;
        let mut ssl_root_cert_string = None;

        // Remove parameters from the url that typical postgres URL includes (for example, with YugabyteDB),
        // but the tokio-rust-postgres driver doesn't support yet.
        // Instead capture those parameters and use them later in the connection/ssl config.
        let query_pairs = url.query_pairs().filter(|(name, value)| {
            if name == "ssl" {
                ssl_param_string = Some(value.to_string());
                false
            } else if name == "sslmode" {
                ssl_mode_string = Some(value.to_string());
                false
            } else if name == "sslrootcert" {
                ssl_root_cert_string = Some(value.to_string());
                false
            } else {
                true
            }
        });

        let mut cleaned_url = url.clone();
        cleaned_url
            .query_pairs_mut()
            .clear()
            .extend_pairs(query_pairs);

        // We need to replace '+' (encoded from a space character) with '%20' since the tokio-rust-postgres driver doesn't seem to support
        // the encoding that uses '+' for a space.
        let url = cleaned_url.as_str().replace('+', "%20");

        let mut ssl_mode = SslMode::Prefer;

        // See: https://jdbc.postgresql.org/documentation/head/ssl-client.html
        // 1. "ssl" parameter is a quick way to specify SSL mode. If it is true, then it has the same effect as setting "sslmode" to "verify-full".
        //    So we process this first.
        if let Some(ssl_param) = ssl_param_string {
            let ssl_param_parsed = ssl_param.as_str().parse();
            match ssl_param_parsed {
                Ok(true) => ssl_mode = SslMode::Require,
                Ok(false) => ssl_mode = SslMode::Prefer,
                _ => {
                    return Err(DatabaseError::Config(format!(
                        "Invalid 'ssl' parameter value {ssl_param}. Must be a 'true' or 'false'",
                    )));
                }
            }
        }
        // 2. The tokio-postgres library doesn't have a way to map all possible values of "sslmode", so we pick the nearest stricter mode.
        //    We process this the next to allow any refinement of the SSL mode set through the simpler "ssl" parameter.
        if let Some(ssl_mode_string) = ssl_mode_string {
            match ssl_mode_string.as_str() {
                "verify-full" | "verify-ca" | "require" => ssl_mode = SslMode::Require,
                "prefer" | "allow" => ssl_mode = SslMode::Prefer,
                "disable" => ssl_mode = SslMode::Disable,
                _ => {
                    return Err(DatabaseError::Config(format!(
                        "Invalid 'sslmode' parameter value {ssl_mode_string}"
                    )))
                }
            }
        }

        let ssl_config = if ssl_mode == SslMode::Disable {
            None
        } else {
            Some(SslConfig {
                mode: ssl_mode,
                root_cert_path: ssl_root_cert_string,
            })
        };

        Ok((url, ssl_config))
    }
}
