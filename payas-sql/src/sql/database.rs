use anyhow::{bail, Context, Result};
use std::env;

use openssl::ssl::{SslConnector, SslMethod, SslVerifyMode};
use postgres::{
    types::{FromSqlOwned, ToSql},
    Client, Config,
};
use postgres_openssl::MakeTlsConnector;

use super::ParameterBinding;

fn type_of<T>(_: &T) -> &str {
    std::any::type_name::<T>()
}

const URL_PARAM: &str = "CLAY_DATABASE_URL";
const USER_PARAM: &str = "CLAY_DATABASE_USER";
const PASSWORD_PARAM: &str = "CLAY_DATABASE_PASSWORD";

#[derive(Debug, Clone)]
pub struct Database {
    url: String,
    user: Option<String>,
    password: Option<String>,
}

impl<'a> Database {
    pub fn from_env() -> Result<Self> {
        let url = env::var(URL_PARAM).context("CLAY_DATABASE_URL must be provided")?;
        let user = env::var(USER_PARAM).ok();
        let password = env::var(PASSWORD_PARAM).ok();

        Ok(Self {
            url,
            user,
            password,
        })
    }

    pub fn execute<T: FromSqlOwned>(&self, binding: &ParameterBinding) -> Result<Option<T>> {
        let mut client = self.create_client()?;

        let params: Vec<&(dyn ToSql + Sync)> =
            binding.params.iter().map(|p| (*p).as_pg()).collect();

        println!("Executing: {}", binding.stmt);
        Self::process(&mut client, &binding.stmt, &params[..])
    }

    fn process<T: FromSqlOwned>(
        client: &mut Client,
        query_string: &str,
        params: &[&(dyn ToSql + Sync)],
    ) -> Result<Option<T>> {
        let rows = client
            .query(query_string, params)
            .context("PostgreSQL query failed")?;

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

    pub fn create_client(&self) -> Result<Client> {
        use std::str::FromStr;
        let mut config =
            Config::from_str(&self.url).context("Failed to parse PostgreSQL connection string")?;

        if let Some(user) = &self.user {
            config.user(user);
        }
        if let Some(password) = &self.password {
            config.password(password);
        }

        if config.get_user() == None {
            bail!("Database user must be specified through as a part of CLAY_DATABASE_URL or through CLAY_DATABASE_USER")
        }

        let mut builder = SslConnector::builder(SslMethod::tls())?;
        builder.set_verify(SslVerifyMode::NONE);
        let connector = MakeTlsConnector::new(builder.build());

        Ok(config.connect(connector)?)
    }
}
