// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use exo_sql::{ColumnPath, ColumnPathLink, PhysicalColumn, PhysicalTable};
use postgres_model::{
    column_id::ColumnId,
    column_path::{ColumnIdPath, ColumnIdPathLink},
    subsystem::PostgresSubsystem,
};

pub fn to_column_path<'a>(
    parent_column_id_path: &Option<ColumnIdPath>,
    next_column_id_path_link: &Option<ColumnIdPathLink>,
    system: &'a PostgresSubsystem,
) -> ColumnPath<'a> {
    let mut path: Vec<_> = match parent_column_id_path {
        Some(parent_column_id_path) => parent_column_id_path
            .path
            .iter()
            .map(|link| to_column_path_link(link, system))
            .collect(),
        None => vec![],
    };

    if let Some(next_column_id_path_link) = next_column_id_path_link {
        path.push(to_column_path_link(next_column_id_path_link, system));
    }

    ColumnPath::Physical(path)
}

fn to_column_table(
    column_id: ColumnId,
    system: &PostgresSubsystem,
) -> (&PhysicalColumn, &PhysicalTable) {
    let column = column_id.get_column(system);
    let table = &system
        .tables
        .iter()
        .find(|(_, table)| table.name == column.table_name)
        .map(|(_, table)| table)
        .unwrap_or_else(|| panic!("Table {} not found", column.table_name));

    (column, table)
}

fn to_column_path_link<'a>(
    link: &ColumnIdPathLink,
    system: &'a PostgresSubsystem,
) -> ColumnPathLink<'a> {
    ColumnPathLink {
        self_column: to_column_table(link.self_column_id, system),
        linked_column: link
            .linked_column_id
            .map(|linked_column_id| to_column_table(linked_column_id, system)),
    }
}
