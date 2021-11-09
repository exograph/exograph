use anyhow::{bail, Context, Result};
use once_cell::sync::OnceCell;
use std::env;

use openssl::ssl::{SslConnector, SslMethod, SslVerifyMode};
use postgres::Config;
use postgres_openssl::MakeTlsConnector;
use r2d2::{Pool, PooledConnection};
use r2d2_postgres::PostgresConnectionManager;

const URL_PARAM: &str = "CLAY_DATABASE_URL";
const USER_PARAM: &str = "CLAY_DATABASE_USER";
const PASSWORD_PARAM: &str = "CLAY_DATABASE_PASSWORD";
const CONNECTION_POOL_SIZE_PARAM: &str = "CLAY_CONNECTION_POOL_SIZE";
const CHECK_CONNECTION_ON_STARTUP: &str = "CLAY_CHECK_CONNECTION_ON_STARTUP";

pub struct Database {
    config: Config,
    pool_size: u32,
    pool: OnceCell<Pool<PostgresConnectionManager<MakeTlsConnector>>>,
}

impl<'a> Database {
    // pool_size_override useful when we want to explicitly control the pool size (for example, to 1, when importing database schema)
    pub fn from_env(pool_size_override: Option<u32>) -> Result<Self> {
        let url = env::var(URL_PARAM).context("CLAY_DATABASE_URL must be provided")?;
        let user = env::var(USER_PARAM).ok();
        let password = env::var(PASSWORD_PARAM).ok();
        let pool_size = pool_size_override.unwrap_or_else(|| {
            env::var(CONNECTION_POOL_SIZE_PARAM)
                .ok()
                .map(|pool_str| pool_str.parse::<u32>().unwrap())
                .unwrap_or(10)
        });
        let check_connection = env::var(CHECK_CONNECTION_ON_STARTUP)
            .ok()
            .map(|pool_str| pool_str.parse::<bool>().unwrap())
            .unwrap_or(true);

        Self::from_env_helper(pool_size, check_connection, url, user, password, None)
    }

    pub fn from_env_helper(
        pool_size: u32,
        check_connection: bool,
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

        let pool = OnceCell::new();

        let db = Self {
            config,
            pool_size,
            pool,
        };

        if check_connection {
            db.get_pool()?;
        }

        Ok(db)
    }

    pub fn get_client(
        &self,
    ) -> Result<PooledConnection<PostgresConnectionManager<MakeTlsConnector>>> {
        Ok(self.get_pool()?.get()?)
    }

    fn get_pool(&self) -> Result<&Pool<PostgresConnectionManager<MakeTlsConnector>>> {
        self.pool.get_or_try_init::<_, anyhow::Error>(|| {
            let mut builder = SslConnector::builder(SslMethod::tls())?;
            builder.set_verify(SslVerifyMode::NONE);
            let connector = MakeTlsConnector::new(builder.build());

            let manager = PostgresConnectionManager::new(self.config.clone(), connector);
            let pool_builder = Pool::builder().max_size(self.pool_size);

            Ok(pool_builder.build(manager)?)
        })
    }
}
