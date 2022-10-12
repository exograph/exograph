use std::collections::HashSet;

use crate::{database_error::DatabaseError, PhysicalTable};
use deadpool_postgres::Client;

use super::{issue::WithIssues, op::SchemaOp};

/// Specification for the overall schema.
#[derive(Default, Debug)]
pub struct SchemaSpec {
    pub tables: Vec<PhysicalTable>,
    pub required_extensions: HashSet<String>,
}

impl SchemaSpec {
    pub fn diff<'a>(&'a self, new: &'a SchemaSpec) -> Vec<SchemaOp<'a>> {
        let existing_tables = &self.tables;
        let new_tables = &new.tables;
        let mut changes = vec![];

        // extension removal
        let extensions_to_drop = self
            .required_extensions
            .difference(&new.required_extensions);
        for extension in extensions_to_drop {
            changes.push(SchemaOp::RemoveExtension {
                extension: extension.clone(),
            })
        }

        // extension creation
        let extensions_to_create = new
            .required_extensions
            .difference(&self.required_extensions);
        for extension in extensions_to_create {
            changes.push(SchemaOp::CreateExtension {
                extension: extension.clone(),
            })
        }

        for existing_table in self.tables.iter() {
            // try to find a table with the same name in the new spec
            match new_tables
                .iter()
                .find(|new_table| existing_table.name == new_table.name)
            {
                // table exists, compare columns
                Some(new_table) => changes.extend(existing_table.diff(new_table)),

                // table does not exist, deletion
                None => changes.push(SchemaOp::DeleteTable {
                    table: existing_table,
                }),
            }
        }

        // try to find a table that needs to be created
        for new_table in new.tables.iter() {
            if !existing_tables
                .iter()
                .any(|old_table| new_table.name == old_table.name)
            {
                // new table
                changes.push(SchemaOp::CreateTable { table: new_table })
            }
        }

        changes
    }

    /// Creates a new schema specification from an SQL database.
    pub async fn from_db(client: &Client) -> Result<WithIssues<SchemaSpec>, DatabaseError> {
        // Query to get a list of all the tables in the database
        const QUERY: &str =
            "SELECT table_name FROM information_schema.tables WHERE table_schema = 'public'";

        let mut issues = Vec::new();
        let mut tables = Vec::new();
        let mut required_extensions = HashSet::new();

        for row in client
            .query(QUERY, &[])
            .await
            .map_err(DatabaseError::Delegate)?
        {
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
