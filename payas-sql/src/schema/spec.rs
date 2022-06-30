use std::collections::HashSet;

use crate::PhysicalTable;
use anyhow::{anyhow, Result};
use deadpool_postgres::Client;

use super::issue::WithIssues;
use super::op::SchemaOp;
use super::statement::SchemaStatement;

/// Specification for the overall schema.
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

    /// Merges the schema specification into a single SQL statement.
    pub fn to_sql_string(&self) -> String {
        let mut ops = Vec::new();

        self.required_extensions.iter().for_each(|ext| {
            ops.push(SchemaOp::CreateExtension {
                extension: ext.to_owned(),
            });
        });

        self.table_specs.iter().for_each(|t| {
            ops.push(SchemaOp::CreateTable { table: t });
        });

        let mut all_pre_statements = Vec::new();
        let mut all_statements = Vec::new();
        let mut all_post_statements = Vec::new();

        ops.into_iter().map(|op| op.to_sql()).for_each(
            |SchemaStatement {
                 statement,
                 pre_statements,
                 post_statements,
             }| {
                all_pre_statements.extend(pre_statements);
                all_statements.push(statement);
                all_post_statements.extend(post_statements);
            },
        );

        all_pre_statements
            .into_iter()
            .chain(all_statements.into_iter())
            .chain(all_post_statements.into_iter())
            .collect::<Vec<_>>()
            .join("\n")
    }
}
