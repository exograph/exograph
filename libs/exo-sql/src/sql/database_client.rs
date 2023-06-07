// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::{cell::RefCell, env};

use deadpool_postgres::{Client, Manager, ManagerConfig, Pool, RecyclingMethod};
use openssl::ssl::{SslConnector, SslMethod, SslVerifyMode};
use postgres_openssl::MakeTlsConnector;
use tokio_postgres::{config::SslMode, Config};

use crate::database_error::DatabaseError;

const URL_PARAM: &str = "EXO_POSTGRES_URL";
const USER_PARAM: &str = "EXO_POSTGRES_USER";
const PASSWORD_PARAM: &str = "EXO_POSTGRES_PASSWORD";
const CONNECTION_POOL_SIZE_PARAM: &str = "EXO_CONNECTION_POOL_SIZE";
const CHECK_CONNECTION_ON_STARTUP: &str = "EXO_CHECK_CONNECTION_ON_STARTUP";
const SSL_VERIFY_PARAM: &str = "EXO_SSL_VERIFY"; // boolean (default: true)

// we spawn many resolvers concurrently in integration tests
thread_local! {
    pub static LOCAL_URL: RefCell<Option<String>> = RefCell::new(None);
    pub static LOCAL_CONNECTION_POOL_SIZE: RefCell<Option<usize>> = RefCell::new(None);
    pub static LOCAL_CHECK_CONNECTION_ON_STARTUP: RefCell<Option<bool>> = RefCell::new(None);
}

pub struct DatabaseClient {
    pool: Pool,
}

struct SslConfig {
    mode: SslMode,
    verify_mode: SslVerifyMode,
    cert_path: Option<String>,
}

impl<'a> DatabaseClient {
    // pool_size_override useful when we want to explicitly control the pool size (for example, to 1, when importing database schema)
    pub async fn from_env(pool_size_override: Option<usize>) -> Result<Self, DatabaseError> {
        let url = LOCAL_URL
            .with(|f| f.borrow().clone())
            .or_else(|| env::var(URL_PARAM).ok())
            .ok_or(DatabaseError::Config(format!(
                "Env {URL_PARAM} must be provided"
            )))?;

        let user = env::var(USER_PARAM).ok();
        let password = env::var(PASSWORD_PARAM).ok();
        let pool_size = pool_size_override.unwrap_or_else(|| {
            LOCAL_CONNECTION_POOL_SIZE
                .with(|f| *f.borrow())
                .or_else(|| {
                    env::var(CONNECTION_POOL_SIZE_PARAM)
                        .ok()
                        .map(|pool_str| pool_str.parse::<usize>().unwrap())
                })
                .unwrap_or(10)
        });

        let check_connection = LOCAL_CHECK_CONNECTION_ON_STARTUP
            .with(|f| *f.borrow())
            .or_else(|| {
                env::var(CHECK_CONNECTION_ON_STARTUP)
                    .ok()
                    .map(|check| check.parse::<bool>().expect("Should be true or false"))
            })
            .unwrap_or(true);

        Self::from_helper(pool_size, check_connection, &url, user, password).await
    }

    pub async fn from_db_url(url: &str) -> Result<Self, DatabaseError> {
        Self::from_helper(1, true, url, None, None).await
    }

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

        if config.get_user().is_none() {
            return Err(DatabaseError::Config("Database user must be specified as a part of EXO_POSTGRES_URL or through EXO_POSTGRES_USER".into()));
        }

        let manager_config = ManagerConfig {
            recycling_method: RecyclingMethod::Fast,
        };

        let manager = match ssl_config {
            Some(SslConfig {
                mode,
                verify_mode,
                cert_path,
            }) => {
                config.ssl_mode(mode);
                let mut builder = SslConnector::builder(SslMethod::tls())?;
                builder.set_verify(verify_mode);
                if let Some(ssl_root_cert) = cert_path {
                    builder.set_ca_file(&ssl_root_cert)?;
                }
                let connector = MakeTlsConnector::new(builder.build());
                Manager::from_config(config, connector, manager_config)
            }
            None => Manager::from_config(config, tokio_postgres::NoTls, manager_config),
        };

        let pool = Pool::builder(manager)
            .max_size(pool_size)
            .build()
            .expect("Failed to create DB pool");

        let db = Self { pool };

        if check_connection {
            let _ = db.get_client().await?;
        }

        Ok(db)
    }

    pub async fn get_client(&self) -> Result<Client, DatabaseError> {
        Ok(self.pool.get().await?)
    }

    fn create_ssl_config(url: &str) -> Result<(String, Option<SslConfig>), DatabaseError> {
        let url = url::Url::parse(url)
            .map_err(|_| DatabaseError::Config("Invalid database URL".into()))?;

        let mut ssl_param_string: Option<String> = None;
        let mut ssl_mode_string: Option<String> = None;
        let mut ssl_root_cert_string = None;

        // Remove parameters from the url that typical postgres URL includes (for example, with YugabyteDB),
        // but the tokio-rust-postgres driver doesn't support yet.
        // Instead capture those parameters and use them later in the connection/ssl config.
        let query = url.query_pairs().filter(|(name, value)| {
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
        cleaned_url.query_pairs_mut().clear().extend_pairs(query);

        // We need to replace '+' (encoded from a space character) with '%20' since the tokio-rust-postgres driver doesn't seem to support
        // the encoding that uses '+' for a space.
        let url = cleaned_url.as_str().replace('+', "%20");

        let mut ssl_mode = SslMode::Disable;

        // See: https://jdbc.postgresql.org/documentation/head/ssl-client.html
        // 1. "ssl" parameter is a quick way to specify SSL mode. If it is true, then it has the same effect as setting "sslmode" to "verify-full".
        //    So we process this first.
        if let Some(ssl_param) = ssl_param_string {
            let ssl_param_parsed = ssl_param.as_str().parse();
            match ssl_param_parsed {
                Ok(true) => ssl_mode = SslMode::Require,
                Ok(false) => ssl_mode = SslMode::Disable,
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

        let ssl_verify = env::var(SSL_VERIFY_PARAM)
            .ok()
            .map(|env_str| match env_str.parse::<bool>() {
                Ok(b) => Ok(b),
                Err(_) => Err(DatabaseError::Config(format!(
                    "Invalid {SSL_VERIFY_PARAM} value: {env_str}. It must be set to 'true' or 'false'",
                ))),
            })
            .unwrap_or(Ok(true))?;

        let ssl_config = if ssl_mode == SslMode::Disable {
            None
        } else {
            Some(SslConfig {
                mode: ssl_mode,
                verify_mode: if ssl_verify {
                    SslVerifyMode::PEER
                } else {
                    SslVerifyMode::NONE
                },
                cert_path: ssl_root_cert_string,
            })
        };

        Ok((url, ssl_config))
    }
}
