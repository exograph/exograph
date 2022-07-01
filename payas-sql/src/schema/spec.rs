use std::collections::HashSet;

use crate::PhysicalTable;
use anyhow::{anyhow, Result};
use deadpool_postgres::Client;

use super::issue::WithIssues;

/// Specification for the overall schema.
#[derive(Default)]
pub struct SchemaSpec {
    pub tables: Vec<PhysicalTable>,
    pub required_extensions: HashSet<String>,
}

impl SchemaSpec {
    /// Creates a new schema specification from an SQL database.
    pub async fn from_db(client: &Client) -> Result<WithIssues<SchemaSpec>> {
        // Query to get a list of all the tables in the database
        const QUERY: &str =
            "SELECT table_name FROM information_schema.tables WHERE table_schema = 'public'";

        let mut issues = Vec::new();
        let mut tables = Vec::new();
        let mut required_extensions = HashSet::new();

        for row in client.query(QUERY, &[]).await.map_err(|e| anyhow!(e))? {
            let name: String = row.get("table_name");
            let mut table = PhysicalTable::from_db(client, &name).await?;
            issues.append(&mut table.issues);
            tables.push(table.value);
        }

        for table in tables.iter() {
            required_extensions = required_extensions
                .union(&table.get_required_extensions())
                .cloned()
                .collect();
        }

        Ok(WithIssues {
            value: SchemaSpec {
                tables,
                required_extensions,
            },
            issues,
        })
    }

    /// Creates a new schema specification from the tables of a claytip model file.
    pub fn from_model(tables: Vec<PhysicalTable>) -> Self {
        let mut required_extensions = HashSet::new();
        for table_spec in tables.iter() {
            required_extensions = required_extensions
                .union(&table_spec.get_required_extensions())
                .cloned()
                .collect();
        }

        SchemaSpec {
            tables,
            required_extensions,
        }
    }
}
