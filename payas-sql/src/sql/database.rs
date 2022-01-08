use anyhow::{bail, Context, Result};
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
        let check_connection = env::var(CHECK_CONNECTION_ON_STARTUP)
            .ok()
            .map(|pool_str| pool_str.parse::<bool>().unwrap())
            .unwrap_or(true);

        Self::from_env_helper(pool_size, check_connection, url, user, password, None)
    }

    pub fn from_env_helper(
        pool_size: usize,
        _check_connection: bool,
        url: String,
        user: Option<String>,
        password: Option<String>,
        db_name_override: Option<String>,
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
        if let Some(db_name) = &db_name_override {
            config.dbname(db_name);
        }

        if config.get_user() == None {
            bail!("Database user must be specified through as a part of CLAY_DATABASE_URL or through CLAY_DATABASE_USER")
        }

        let mut builder = SslConnector::builder(SslMethod::tls())?;
        builder.set_verify(SslVerifyMode::NONE);
        let connector = MakeTlsConnector::new(builder.build());

        let manager = Manager::from_config(
            config,
            connector, // or tokio_postgres::NoTls,
            ManagerConfig {
                recycling_method: RecyclingMethod::Fast,
            },
        );

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
}
