// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use crate::{database_error::DatabaseError, Database, PhysicalTable};
use deadpool_postgres::Client;

use super::{issue::WithIssues, op::SchemaOp};

pub fn diff<'a>(old: &'a Database, new: &'a Database) -> Vec<SchemaOp<'a>> {
    let mut changes = vec![];

    let old_required_extensions = old.required_extensions();
    let new_required_extensions = new.required_extensions();

    // extension removal
    let extensions_to_drop = old_required_extensions.difference(&new_required_extensions);
    for extension in extensions_to_drop {
        changes.push(SchemaOp::RemoveExtension {
            extension: extension.clone(),
        })
    }

    // extension creation
    let extensions_to_create = new_required_extensions.difference(&old_required_extensions);
    for extension in extensions_to_create {
        changes.push(SchemaOp::CreateExtension {
            extension: extension.clone(),
        })
    }

    let old_tables = old.tables().iter().map(|(_, table)| table);
    let mut new_tables = new.tables().iter().map(|(_, table)| table);

    for old_table in old_tables {
        // try to find a table with the same name in the new spec
        match new_tables.find(|new_table| old_table.name == new_table.name) {
            // table exists, compare columns
            Some(new_table) => changes.extend(old_table.diff(new_table)),

            // table does not exist, deletion
            None => changes.push(SchemaOp::DeleteTable { table: old_table }),
        }
    }

    let mut old_tables = old.tables().iter().map(|(_, table)| table);
    let new_tables = new.tables().iter().map(|(_, table)| table);
    // try to find a table that needs to be created
    for new_table in new_tables {
        if !old_tables.any(|old_table| new_table.name == old_table.name) {
            // new table
            changes.push(SchemaOp::CreateTable { table: new_table })
        }
    }

    changes
}

/// Creates a new schema specification from an SQL database.
pub async fn from_db(client: &Client) -> Result<WithIssues<Database>, DatabaseError> {
    // Query to get a list of all the tables in the database
    const QUERY: &str =
        "SELECT table_name FROM information_schema.tables WHERE table_schema = 'public'";

    let mut database: Database = Default::default();
    let mut issues = Vec::new();

    for row in client
        .query(QUERY, &[])
        .await
        .map_err(DatabaseError::Delegate)?
    {
        let name: String = row.get("table_name");
        let table_id = database.insert_table(PhysicalTable {
            name,
            columns: vec![],
        });
        let mut table = PhysicalTable::from_db(client, table_id, &database).await?;
        issues.append(&mut table.issues);
        database.insert_table(table.value);
    }

    Ok(WithIssues {
        value: database,
        issues,
    })
}
