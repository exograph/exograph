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
        for (table_id, column_specs) in tables.iter() {
            let columns = column_specs
                .iter()
                .map(|column_spec| PhysicalColumn {
                    table_id: *table_id,
                    name: column_spec.name.to_owned(),
                    typ: column_spec.typ.to_database_type(), // This will set typ to a placeholder value for reference columns
                    is_pk: column_spec.is_pk,
                    is_auto_increment: column_spec.is_auto_increment,
                    is_nullable: column_spec.is_nullable,
                    unique_constraints: column_spec.unique_constraints.to_owned(),
                    default_value: column_spec.default_value.to_owned(),
                })
                .collect();

            database.get_table_mut(*table_id).columns = columns;
        }

        // Step 3: Set column types. We have to perform this in a separate step because we need all tables and columns to exists
        //         for us to be able to get the column ids.
        let updates: Vec<_> = tables
            .iter()
            .flat_map(|(table_id, column_specs)| {
                let table = database.get_table(*table_id);

                table.columns.iter().flat_map(|column| {
                    let column_spec = column_specs
                        .iter()
                        .find(|column_spec| column_spec.name == column.name)
                        .unwrap();

                    let column_id = database.get_column_id(*table_id, &column.name).unwrap();
                    column_spec
                        .typ
                        .to_database_reference_type(&database)
                        .map(|typ| (column_id, typ))
                })
            })
            .collect();

        for (column_id, typ) in updates {
            let table = database.get_table_mut(column_id.table_id);
            table.columns[column_id.column_index].typ = typ;
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
            let mut table = TableSpec::from_live_db(client, &table_name).await?;
            issues.append(&mut table.issues);
            tables.push(table.value);
        }

        Ok(WithIssues {
            value: DatabaseSpec { tables },
            issues,
        })
    }
}
