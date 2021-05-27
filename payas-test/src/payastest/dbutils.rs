use postgres::NoTls;
use std::error::Error;
use postgres::{config::Host, Client, Config};

/// Create a database with the specified name at the specified PostgreSQL server and return 
/// a connection string for the database on successful creation.
pub fn createdb_psql(dbname: &str, url: &str) -> Result<String, Box<dyn Error>> {
    // TODO validate dbname

    let config = url.parse::<Config>()?;
    let mut client: Client = config.connect(NoTls)?;

    let query: String = format!("CREATE DATABASE {}", dbname);
    client.execute(query.as_str() , &[])?;

    // start building our connection string
    let mut connectionparams = 
        "postgresql://".to_string() + 
        config.get_user().unwrap();

    // add password if necessary
    match config.get_password() {
        Some(password) => {
            connectionparams += &(":".to_string() + std::str::from_utf8(password).unwrap());
        },

        None => {}
    }

    // add host
    match &config.get_hosts()[0] {
        Host::Tcp(host) => {
            connectionparams += &("@".to_string() + &host);
        },

        // TODO Unix sockets
        Host::Unix(_) => {}
    }

    // add db
    connectionparams += &("/".to_string() + dbname);

    println!("{}", connectionparams);

    // return
    Ok(connectionparams)
}

/// Drop the specified database at the specified PostgreSQL server and
/// return on success.
pub fn dropdb_psql(dbname: &str, url: &str) -> Result<(), Box<dyn Error>> {
    let mut client = url.parse::<Config>()?.connect(NoTls)?;

    let query: String = format!("DROP DATABASE {}", dbname);
    client.execute(query.as_str() , &[])?;

    Ok(())
}

