use anyhow::{bail, Context, Result};
use std::env;

use openssl::ssl::{SslConnector, SslMethod, SslVerifyMode};
use postgres::{
    types::{FromSqlOwned, ToSql},
    Config, Row,
};
use postgres_openssl::MakeTlsConnector;
use r2d2::{Pool, PooledConnection};
use r2d2_postgres::PostgresConnectionManager;

use super::ParameterBinding;

fn type_of<T>(_: &T) -> &str {
    std::any::type_name::<T>()
}

const URL_PARAM: &str = "CLAY_DATABASE_URL";
const USER_PARAM: &str = "CLAY_DATABASE_USER";
const PASSWORD_PARAM: &str = "CLAY_DATABASE_PASSWORD";

#[derive(Clone)]
pub struct Database {
    url: String,
    user: Option<String>,
    password: Option<String>,
    pool: Pool<PostgresConnectionManager<MakeTlsConnector>>,
}

impl<'a> Database {
    pub fn from_env(pool_size: u32) -> Result<Self> {
        let url = env::var(URL_PARAM).context("CLAY_DATABASE_URL must be provided")?;
        let user = env::var(USER_PARAM).ok();
        let password = env::var(PASSWORD_PARAM).ok();

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

        let mut builder = SslConnector::builder(SslMethod::tls())?;
        builder.set_verify(SslVerifyMode::NONE);
        let connector = MakeTlsConnector::new(builder.build());

        let manager = PostgresConnectionManager::new(config, connector);
        let pool = Pool::builder().max_size(pool_size).build(manager)?;

        Ok(Self {
            url,
            user,
            password,
            pool,
        })
    }

    pub fn execute<T: FromSqlOwned>(
        &self,
        binding: &ParameterBinding,
        extractor: fn(Vec<Row>) -> Result<Option<T>>,
    ) -> Result<Option<T>> {
        let mut client = self.get_client()?;

        let params: Vec<&(dyn ToSql + Sync)> =
            binding.params.iter().map(|p| (*p).as_pg()).collect();

        println!("Executing: {}", binding.stmt);
        let rows = client
            .query(binding.stmt.as_str(), &params[..])
            .context("PostgreSQL query failed")?;
        let extracted = extractor(rows)?;

        Ok(extracted.into_iter().next())
    }

    pub fn get_client(
        &self,
    ) -> Result<PooledConnection<PostgresConnectionManager<MakeTlsConnector>>> {
        Ok(self.pool.get()?)
    }
}

pub fn extractor_single<T: FromSqlOwned>(rows: Vec<Row>) -> Result<Option<T>> {
    let result = if rows.len() == 1 {
        match rows[0].try_get(0) {
            Ok(col) => Some(col),
            Err(err) => bail!("Got row without any columns {}", err),
        }
    } else {
        None
    };

    Ok(result)
}

pub fn extractor_vec<T: FromSqlOwned>(rows: Vec<Row>) -> Result<Vec<T>> {
    Ok(rows
        .into_iter()
        .flat_map(|row| match row.try_get(0) {
            Ok(col) => Ok(col),
            Err(err) => bail!("Got row without any columns {}", err),
        })
        .collect())
}
