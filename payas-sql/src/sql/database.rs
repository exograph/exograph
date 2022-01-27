use anyhow::{anyhow, bail, Context, Result};
use std::env;

use deadpool_postgres::{Client, Manager, ManagerConfig, Pool, RecyclingMethod};
use openssl::ssl::{SslConnector, SslMethod, SslVerifyMode};
use postgres_openssl::MakeTlsConnector;
use tokio_postgres::Config;

const URL_PARAM: &str = "CLAY_DATABASE_URL";
const USER_PARAM: &str = "CLAY_DATABASE_USER";
const PASSWORD_PARAM: &str = "CLAY_DATABASE_PASSWORD";
const CONNECTION_POOL_SIZE_PARAM: &str = "CLAY_CONNECTION_POOL_SIZE";
const CHECK_CONNECTION_ON_STARTUP: &str = "CLAY_CHECK_CONNECTION_ON_STARTUP";
const SSL_METHOD_PARAM: &str = "CLAY_SSL_METHOD"; // Possible values: "none" (default), "tls", "dtls", "tls_client", and "tls_server"
const SSL_NO_VERIFY_PARAM: &str = "CLAY_SSL_NO_VERIFY"; // boolean (default: false)

pub struct Database {
    pool: Pool,
}

impl<'a> Database {
    // pool_size_override useful when we want to explicitly control the pool size (for example, to 1, when importing database schema)
    pub fn from_env(pool_size_override: Option<usize>) -> Result<Self> {
        let url = env::var(URL_PARAM).context("CLAY_DATABASE_URL must be provided")?;
        let user = env::var(USER_PARAM).ok();
        let password = env::var(PASSWORD_PARAM).ok();
        let pool_size = pool_size_override.unwrap_or_else(|| {
            env::var(CONNECTION_POOL_SIZE_PARAM)
                .ok()
                .map(|pool_str| pool_str.parse::<usize>().unwrap())
                .unwrap_or(10)
        });

        let ssl_config = Self::create_ssl_config()?;

        let check_connection = env::var(CHECK_CONNECTION_ON_STARTUP)
            .ok()
            .map(|pool_str| pool_str.parse::<bool>().unwrap())
            .unwrap_or(true);

        Self::from_env_helper(pool_size, check_connection, url, user, password, ssl_config)
    }

    fn from_env_helper(
        pool_size: usize,
        _check_connection: bool,
        url: String,
        user: Option<String>,
        password: Option<String>,
        ssl_config: Option<(SslMethod, SslVerifyMode)>,
    ) -> Result<Self> {
        use std::str::FromStr;

        let mut config =
            Config::from_str(&url).context("Failed to parse PostgreSQL connection string")?;

        if let Some(user) = &user {
            config.user(user);
        }
        if let Some(password) = &password {
            config.password(password);
        }

        if config.get_user() == None {
            bail!("Database user must be specified through as a part of CLAY_DATABASE_URL or through CLAY_DATABASE_USER")
        }

        let manager_config = ManagerConfig {
            recycling_method: RecyclingMethod::Fast,
        };

        let manager = match ssl_config {
            Some((ssl_method, ssl_verify_mode)) => {
                let mut builder = SslConnector::builder(ssl_method)?;
                builder.set_verify(ssl_verify_mode);
                let connector = MakeTlsConnector::new(builder.build());
                Manager::from_config(config, connector, manager_config)
            }
            None => Manager::from_config(config, tokio_postgres::NoTls, manager_config),
        };

        let pool = Pool::builder(manager).max_size(pool_size).build().unwrap();

        let db = Self { pool };

        // if check_connection {
        //     let _ = db.get_client().await?;
        // }

        Ok(db)
    }

    pub async fn get_client(&self) -> Result<Client> {
        Ok(self.pool.get().await?)
    }

    fn create_ssl_config() -> Result<Option<(SslMethod, SslVerifyMode)>> {
        let ssl_method = env::var(SSL_METHOD_PARAM)
            .ok()
            .map(
                |env_str| match env_str.as_str().to_ascii_lowercase().as_str() {
                    "tls" => Ok(Some(SslMethod::tls())),
                    "dtls" => Ok(Some(SslMethod::dtls())),
                    "tls_client" => Ok(Some(SslMethod::tls_client())),
                    _ => Err(anyhow!(
                        "Invalid SSL method: {}. Env {} must be set to either 'tls', 'dtls', or 'tls_client'", env_str, SSL_METHOD_PARAM
                    )),
                },
            )
            .unwrap_or_else(|| Ok(None))?;

        let ssl_no_verify = env::var(SSL_NO_VERIFY_PARAM)
            .ok()
            .map(|env_str| match env_str.parse::<bool>() {
                Ok(b) => Ok(b),
                Err(_) => Err(anyhow!(
                    "Invalid SSL_NO_VERIFY value: {}. Env {} must be set to true or false",
                    env_str,
                    SSL_NO_VERIFY_PARAM
                )),
            })
            .unwrap_or(Ok(false))?;

        if !ssl_no_verify && ssl_method.is_none() {
            bail!(
                "{} must be set to 'tls', 'dtls', or 'tls_client' when {} is false",
                SSL_METHOD_PARAM,
                SSL_NO_VERIFY_PARAM
            )
        }

        let ssl_config = match (ssl_method, ssl_no_verify) {
            (Some(ssl_method), false) => Some((ssl_method, SslVerifyMode::PEER)),
            (Some(ssl_method), true) => Some((ssl_method, SslVerifyMode::NONE)),
            _ => None,
        };

        Ok(ssl_config)
    }
}
