use std::env;

use openssl::ssl::{SslConnector, SslMethod, SslVerifyMode};
use postgres::{types::ToSql, Client, Config};
use postgres_openssl::MakeTlsConnector;

use super::ParameterBinding;

fn type_of<T>(_: &T) -> &str {
    std::any::type_name::<T>()
}

const URL_PARAM: &'static str = "PAYAS_DATABASE_URL";
const USER_PARAM: &'static str = "PAYAS_DATABASE_USER";
const PASSWORD_PARAM: &'static str = "PAYAS_DATABASE_PASSWORD";

#[derive(Debug, Clone)]
pub struct Database {
    url: String,
    user: Option<String>,
    password: Option<String>,
}

impl<'a> Database {
    pub fn from_env() -> Self {
        let url = env::var(URL_PARAM).expect("PAYAS_DATABASE_URL must be provided");
        let user = env::var(USER_PARAM).ok();
        let password = env::var(PASSWORD_PARAM).ok();

        Self {
            url,
            user,
            password,
        }
    }

    pub fn execute(&self, binding: &ParameterBinding) -> String {
        let mut client = self.create_client();

        let params: Vec<&(dyn ToSql + Sync)> =
            binding.params.iter().map(|p| (*p).as_pg()).collect();

        println!("Executing: {}", binding.stmt);
        Self::process(&mut client, &binding.stmt, &params[..])
    }

    fn process(client: &mut Client, query_string: &str, params: &[&(dyn ToSql + Sync)]) -> String {
        let rows = client.query(query_string, params).unwrap();

        if rows.len() == 1 {
            match rows[0].try_get(0) {
                Ok(col) => col,
                Err(err) => panic!("Got row without any columns {}", err),
            }
        } else {
            // TODO: Check if "null" is right
            "null".to_owned()
        }
    }

    pub fn create_client(&self) -> Client {
        use std::str::FromStr;
        let mut config = Config::from_str(&self.url).unwrap();

        if let Some(user) = &self.user {
            config.user(user);
        }
        if let Some(password) = &self.password {
            config.password(password);
        }

        if config.get_user() == None {
            panic!("Database user must be specified through as a part of PAYAS_DATABASE_URL or through PAYAS_DATABASE_USER")
        }

        let mut builder = SslConnector::builder(SslMethod::tls()).unwrap();
        builder.set_verify(SslVerifyMode::NONE);
        let connector = MakeTlsConnector::new(builder.build());

        config.connect(connector).unwrap()
    }
}
