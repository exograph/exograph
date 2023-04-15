use anyhow::{Context, Result};
use exo_sql::testing::db::EphemeralDatabase;
use postgres::Config;
use postgres::NoTls;

/// Connect to the specified PostgreSQL database and attempt to run a query.
pub(crate) fn run_psql(query: &str, db: &(dyn EphemeralDatabase + Send + Sync)) -> Result<()> {
    let mut client = db.url().parse::<Config>()?.connect(NoTls)?;
    client
        .simple_query(query)
        .context(format!("PostgreSQL query failed: {query}"))
        .map(|_| ())
}
