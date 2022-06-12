use anyhow::{anyhow, bail, Context, Result};
use std::env;

use deadpool_postgres::{Client, Manager, ManagerConfig, Pool, RecyclingMethod};
use openssl::ssl::{SslConnector, SslMethod, SslVerifyMode};
use postgres_openssl::MakeTlsConnector;
use tokio_postgres::{config::SslMode, Config};

const URL_PARAM: &str = "CLAY_DATABASE_URL";
const USER_PARAM: &str = "CLAY_DATABASE_USER";
const PASSWORD_PARAM: &str = "CLAY_DATABASE_PASSWORD";
const CONNECTION_POOL_SIZE_PARAM: &str = "CLAY_CONNECTION_POOL_SIZE";
const CHECK_CONNECTION_ON_STARTUP: &str = "CLAY_CHECK_CONNECTION_ON_STARTUP";
const SSL_METHOD_PARAM: &str = "CLAY_SSL_METHOD"; // Possible values: "tls" and "dtls"
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
        url_string: String,
        user: Option<String>,
        password: Option<String>,
        ssl_config: Option<(SslMethod, SslVerifyMode)>,
    ) -> Result<Self> {
        use std::str::FromStr;

        let url = url::Url::parse(&url_string).context("Invalid URL")?;

        let mut ssl_param: Option<String> = None;
        let mut ssl_mode: Option<String> = None;
        let mut ssl_root_cert = None;

        // Remove parameters from the url that typical postgres URL includes (for example, with YugabyteDB),
        // but the tokio-rust-postgres driver doesn't support yet.
        // Instead capture those parameters and use them later in the connection/ssl config.
        let query = url.query_pairs().filter(|(name, value)| {
            if name == "ssl" {
                ssl_param = Some(value.to_string());
                false
            } else if name == "sslmode" {
                ssl_mode = Some(value.to_string());
                false
            } else if name == "sslrootcert" {
                ssl_root_cert = Some(value.to_string());
                false
            } else {
                true
            }
        });

        let mut cleaned_url = url.clone();
        cleaned_url.query_pairs_mut().clear().extend_pairs(query);

        // We need to replace '+' (encoded from a space character) with '%20' since the tokio-rust-postgres driver doesn't seem to support
        // the encoding that uses '+' for a space.
        let mut config = Config::from_str(cleaned_url.as_str().replace('+', "%20").as_str())
            .context("Failed to parse PostgreSQL connection string")?;

        if let Some(user) = &user {
            config.user(user);
        }
        if let Some(password) = &password {
            config.password(password);
        }

        if config.get_user() == None {
            bail!("Database user must be specified through as a part of CLAY_DATABASE_URL or through CLAY_DATABASE_USER")
        }

        // See: https://jdbc.postgresql.org/documentation/head/ssl-client.html
        // 1. "ssl" parameter is a quick way to specify SSL mode. If it is true, then it has the same effect as setting "sslmode" to "verify-full".
        //    So we process this first.
        if let Some(ssl_param) = ssl_param {
            let ssl_mode = match ssl_param.as_str() {
                "true" => SslMode::Require,
                "false" => SslMode::Disable,
                _ => bail!("Invalid 'ssl' parameter value {ssl_param}"),
            };
            config.ssl_mode(ssl_mode);
        }
        // 2. The tokio-postgres library doesn't have a way to map all possible values of "sslmode", so we pick the nearest stricter mode.
        //    We process this the next to allow any refinement of the SSL mode set through the simpler "ssl" parameter.
        if let Some(ssl_mode) = ssl_mode {
            let ssl_mode = match ssl_mode.as_str() {
                "verify-full" | "verify-ca" | "require" => SslMode::Require,
                "prefer" | "allow" => SslMode::Prefer,
                "disable" => SslMode::Disable,
                _ => bail!("Invalid 'sslmode' parameter value {ssl_mode}"),
            };
            config.ssl_mode(ssl_mode);
        }

        let manager_config = ManagerConfig {
            recycling_method: RecyclingMethod::Fast,
        };

        let manager = match ssl_config {
            Some((ssl_method, ssl_verify_mode)) => {
                let mut builder = SslConnector::builder(ssl_method)?;
                builder.set_verify(ssl_verify_mode);
                if let Some(ssl_root_cert) = ssl_root_cert {
                    builder.set_ca_file(&ssl_root_cert)?;
                }
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
                    _ => Err(anyhow!(
                        "Invalid SSL method: {}. Env {} must be set to either 'tls' or 'dtls'",
                        env_str,
                        SSL_METHOD_PARAM
                    )),
                },
            )
            .unwrap_or_else(|| Ok(None))?;

        let ssl_no_verify = env::var(SSL_NO_VERIFY_PARAM)
            .ok()
            .map(|env_str| match env_str.parse::<bool>() {
                Ok(b) => Ok(Some(b)),
                Err(_) => Err(anyhow!(
                    "Invalid {} value: {}. It must be set to 'true' or 'false'",
                    SSL_NO_VERIFY_PARAM,
                    env_str,
                )),
            })
            .unwrap_or_else(|| Ok(None))?;

        if ssl_method.is_none() && ssl_no_verify == Some(false) {
            bail!(
                "{} must be set to 'tls' or 'dtls' when {} is set to 'false'",
                SSL_METHOD_PARAM,
                SSL_NO_VERIFY_PARAM
            )
        }

        let ssl_config = match (ssl_method, ssl_no_verify) {
            (Some(ssl_method), Some(false)) => Some((ssl_method, SslVerifyMode::PEER)),
            (Some(ssl_method), Some(true)) => Some((ssl_method, SslVerifyMode::NONE)),
            _ => None,
        };

        Ok(ssl_config)
    }
}
