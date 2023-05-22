// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::collections::HashSet;

use deadpool_postgres::Client;

use crate::{
    database_error::DatabaseError, schema::column_spec::ColumnSpec, Database, PhysicalColumn,
    PhysicalTable,
};

use super::{issue::WithIssues, table_spec::TableSpec};

pub struct DatabaseSpec {
    pub tables: Vec<TableSpec>,
}

impl DatabaseSpec {
    pub fn new(tables: Vec<TableSpec>) -> Self {
        Self { tables }
    }
}

impl DatabaseSpec {
    pub fn required_extensions(&self) -> HashSet<String> {
        self.tables.iter().fold(HashSet::new(), |acc, table| {
            acc.union(&table.get_required_extensions())
                .cloned()
                .collect()
        })
    }

    pub fn to_database(self) -> Database {
        let mut database = Database::default();

        // Step 1: Create tables (without columns)
        let tables: Vec<_> = self
            .tables
            .into_iter()
            .map(|table| {
                let table_id = database.insert_table(PhysicalTable {
                    name: table.name,
                    columns: vec![],
                });
                (table_id, table.columns)
            })
            .collect();

        // Step 2: Add columns to tables
        for (table_id, column_specs) in tables.into_iter() {
            let columns = column_specs
                .into_iter()
                .map(|column_spec| PhysicalColumn {
                    table_id,
                    name: column_spec.name,
                    typ: column_spec.typ.to_database_type(),
                    is_pk: column_spec.is_pk,
                    is_auto_increment: column_spec.is_auto_increment,
                    is_nullable: column_spec.is_nullable,
                    unique_constraints: column_spec.unique_constraints,
                    default_value: column_spec.default_value,
                })
                .collect();

            database.get_table_mut(table_id).columns = columns;
        }

        database
    }

    pub fn from_database(database: Database) -> DatabaseSpec {
        let tables = database
            .tables()
            .into_iter()
            .map(|(_, table)| TableSpec {
                name: table.name.clone(),
                columns: table
                    .columns
                    .clone()
                    .into_iter()
                    .map(ColumnSpec::from_physical)
                    .collect(),
            })
            .collect();

        DatabaseSpec { tables }
    }

    /// Creates a new schema specification from an SQL database.
    pub async fn from_live_database(
        client: &Client,
    ) -> Result<WithIssues<DatabaseSpec>, DatabaseError> {
        // Query to get a list of all the tables in the database
        const QUERY: &str =
            "SELECT table_name FROM information_schema.tables WHERE table_schema = 'public'";

        let mut issues = Vec::new();
        let mut tables = Vec::new();

        for row in client
            .query(QUERY, &[])
            .await
            .map_err(DatabaseError::Delegate)?
        {
            let table_name: String = row.get("table_name");
            let mut table = TableSpec::from_db(client, &table_name).await?;
            issues.append(&mut table.issues);
            tables.push(table.value);
        }

        Ok(WithIssues {
            value: DatabaseSpec { tables },
            issues,
        })
    }
}
