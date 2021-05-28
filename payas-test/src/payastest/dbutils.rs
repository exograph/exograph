use postgres::NoTls;
use std::error::Error;
use postgres::{config::Host, Client, Config};
use simple_error::{SimpleError, bail};

/// Create a database with the specified name at the specified PostgreSQL server and return 
/// a connection string for the database on successful creation.
pub fn createdb_psql(dbname: &str, url: &str) -> Result<(String, String), Box<dyn Error>> {
    // TODO validate dbname

    let config = url.parse::<Config>()?;
    let mut client: Client = config.connect(NoTls)?;

    let username = config.get_user().ok_or(SimpleError::new("No user specified in configuration"))?;

    let query: String = format!("CREATE DATABASE {}", dbname);
    client.execute(query.as_str() , &[])?;

    // start building our connection string
    let mut connectionparams = String::from("postgresql://".to_string() + username);

    // add password if given
    match config.get_password() {
        Some(password) => {
            connectionparams += &(":".to_string() + std::str::from_utf8(password).unwrap());
        },

        None => {}
    }

    // add host
    match &config.get_hosts().get(0) {
        Some(host) => match host {
            Host::Tcp(host) => {
                connectionparams += &("@".to_string() + &host);
            },

            // TODO Unix sockets
            Host::Unix(_) => {
                bail!("Unix socket connections to PostgreSQL are currently not supported.")
            }
        },
        None => bail!("No host specified.")
    }

    // add db
    connectionparams += &("/".to_string() + dbname);
    //println!("{}", connectionparams);

    // return
    Ok((connectionparams, username.to_owned()))
}

/// Drop the specified database at the specified PostgreSQL server and
/// return on success.
pub fn dropdb_psql(dbname: &str, url: &str) -> Result<(), Box<dyn Error>> {
    let mut client = url.parse::<Config>()?.connect(NoTls)?;

    let query: String = format!("DROP DATABASE {}", dbname);
    client.execute(query.as_str() , &[])?;

    Ok(())
}

