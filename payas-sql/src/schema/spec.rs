use std::collections::HashSet;

use crate::PhysicalTable;
use anyhow::{anyhow, Result};
use deadpool_postgres::Client;

use super::issue::WithIssues;

/// Specification for the overall schema.
#[derive(Default)]
pub struct SchemaSpec {
    pub table_specs: Vec<PhysicalTable>,
    pub required_extensions: HashSet<String>,
}

impl SchemaSpec {
    /// Creates a new schema specification from an SQL database.
    pub async fn from_db(client: &Client) -> Result<WithIssues<SchemaSpec>> {
        // Query to get a list of all the tables in the database
        const QUERY: &str =
            "SELECT table_name FROM information_schema.tables WHERE table_schema = 'public'";

        let mut issues = Vec::new();
        let mut table_specs = Vec::new();
        let mut required_extensions = HashSet::new();

        for row in client.query(QUERY, &[]).await.map_err(|e| anyhow!(e))? {
            let name: String = row.get("table_name");
            let mut table = PhysicalTable::from_db(client, &name).await?;
            issues.append(&mut table.issues);
            table_specs.push(table.value);
        }

        for table_spec in table_specs.iter() {
            required_extensions = required_extensions
                .union(&table_spec.get_required_extensions())
                .cloned()
                .collect();
        }

        Ok(WithIssues {
            value: SchemaSpec {
                table_specs,
                required_extensions,
            },
            issues,
        })
    }
}
