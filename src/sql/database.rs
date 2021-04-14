use id_arena::Arena;
use openssl::ssl::{SslConnector, SslMethod, SslVerifyMode};
use postgres::{types::ToSql, Client};
use postgres_openssl::MakeTlsConnector;

use super::{table::PhysicalTable, ParameterBinding};

fn type_of<T>(_: &T) -> &str {
    std::any::type_name::<T>()
}

#[derive(Debug, Clone)]
pub struct Database {
    pub tables: Arena<PhysicalTable>,
}

impl<'a> Database {
    pub fn empty() -> Self {
        Self {
            tables: Arena::new(),
        }
    }

    pub fn execute(&self, binding: &ParameterBinding) -> String {
        let mut client = Self::create_client();

        let params: Vec<&(dyn ToSql + Sync)> =
            binding.params.iter().map(|p| (*p).as_pg()).collect();

        println!("Executing: {}", binding.stmt);
        Self::process(&mut client, &binding.stmt, &params[..])
    }

    fn process(client: &mut Client, query_string: &str, params: &[&(dyn ToSql + Sync)]) -> String {
        let rows = client.query(query_string, params).unwrap();

        // TODO: Check if "null" is right
        if rows.len() == 1 {
            match rows[0].try_get(0) {
                Ok(col) => col,
                _ => panic!("Got row without any columns"),
            }
        } else {
            "null".to_owned()
        }
    }

    fn create_client() -> Client {
        let host = "localhost";
        let port = 5432;
        let name = "payas-test";
        let user = "postgres";
        let password = "postgres";

        let url = format!(
            "host={} port={} dbname={} user={} password={}",
            host, port, name, user, password,
        );

        let mut builder = SslConnector::builder(SslMethod::tls()).unwrap();
        builder.set_verify(SslVerifyMode::NONE); // DO's self-signed cert doesn't work, so don't do SSL verification
        let connector = MakeTlsConnector::new(builder.build());

        postgres::Client::connect(&url, connector).unwrap()
    }
}
