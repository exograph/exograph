use anyhow::{bail, Context, Result};
use postgres::NoTls;
use postgres::{config::Host, Client, Config};

type ConnectionString = String;
type DbUsername = String;

/// Create a database with the specified name at the specified PostgreSQL server and return
/// a connection string and username for the database on successful creation.
pub fn createdb_psql(dbname: &str, url: &str) -> Result<(ConnectionString, DbUsername)> {
    // TODO validate dbname

    // parse connection string
    let mut config = url
        .parse::<Config>()
        .context("Failed to parse PostgreSQL connection string")?;

    // "The postgres database is a default database meant for use by users, utilities and third party applications."
    config.dbname("postgres");

    // validate and parse out connection parameters
    let username = &config
        .get_user()
        .context("Missing user in connection string")?;
    let password: Option<&str> = config.get_password().map(std::str::from_utf8).transpose()?;
    let host: Option<&str> = config
        .get_hosts()
        .get(0)
        .map(|host| match host {
            Host::Tcp(host) => Ok(host.as_str()),
            Host::Unix(_) => {
                bail!("Unix socket connections are currently not supported")
            }
        })
        .transpose()?;

    // run creation query
    let mut client: Client = config.connect(NoTls)?;
    let query: String = format!("CREATE DATABASE \"{}\"", dbname);
    client
        .execute(query.as_str(), &[])
        .context("PostgreSQL database creation query failed")?;

    // start building our connection string
    let mut connectionparams = "postgresql://".to_string() + username;

    // add password if given
    if let Some(password) = password {
        connectionparams += &(":".to_string() + password);
    }

    // add host
    if let Some(host) = host {
        connectionparams += &("@".to_string() + host);
    } else {
        bail!("No PostgreSQL host specified")
    }

    // add db
    connectionparams += &("/".to_string() + dbname);

    // set a common timezone for tests for consistency
    connectionparams += "?options=-c%20TimeZone%3DUTC%2B00"; // -c TimeZone=UTC+00

    // return
    Ok((connectionparams, username.to_string()))
}

/// Connect to the specified PostgreSQL database and attempt to run a query.
pub fn run_psql(query: &str, url: &str) -> Result<()> {
    let mut client = url.parse::<Config>()?.connect(NoTls)?;
    client
        .simple_query(query)
        .context("PostgreSQL query failed to execute")
        .map(|_| ())
}

/// Drop the specified database at the specified PostgreSQL server and
/// return on success.
pub fn dropdb_psql(dbname: &str, url: &str) -> Result<()> {
    let mut config = url.parse::<Config>()?;

    // "The postgres database is a default database meant for use by users, utilities and third party applications."
    config.dbname("postgres");

    let mut client = config.connect(NoTls)?;

    let query: String = format!("DROP DATABASE \"{}\"", dbname);
    client
        .execute(query.as_str(), &[])
        .context("PostgreSQL drop database query failed")
        .map(|_| ())
}
