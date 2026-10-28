// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use exo_sql_core::{ColumnId, Database};

use crate::{Column, ExpressionBuilder, SQLBuilder};

use crate::core::PgColumnTypeExt;
use crate::core::pg_extension::PgExtension;
use crate::physical_column_type::EnumColumnType;

// Re-export the core Table type specialized to PgExtension
pub type Table = exo_sql_core::operation::Table<PgExtension>;

impl ExpressionBuilder for Table {
    /// Build the table into a SQL string.
    fn build(&self, database: &Database, builder: &mut SQLBuilder) {
        match self {
            Table::Physical { table_id, alias } => {
                let physical_table = database.get_table(*table_id);
                physical_table.build(database, builder);

                if let Some(alias) = alias {
                    // If the table name is the same as the alias (and the table is in the "public" schema), we don't need to alias it
                    if &physical_table.name.name != alias || physical_table.name.schema.is_some() {
                        builder.push_str(" AS ");
                        builder.push_identifier(alias);
                    }
                }
            }
            Table::Join(join) => join.build(database, builder),
            Table::SubSelect { select, alias } => {
                builder.push('(');
                select.build(database, builder);
                builder.push(')');
                if let Some((alias, _)) = alias {
                    builder.push_str(" AS ");
                    builder.push_identifier(alias);
                }
            }
            Table::Values { table_id, row } => {
                let physical_table = database.get_table(*table_id);

                if row.is_empty() {
                    // `VALUES` needs at least one column, but a select without columns is one row
                    builder.push_str("(SELECT)");
                } else {
                    builder.push_str("(VALUES (");
                    builder.push_iter(row.iter(), ", ", |builder, (column_id, value)| {
                        build_row_value(*column_id, value, database, builder)
                    });
                    builder.push_str("))");
                }

                // Aliased as the table itself, so that its columns refer to the values
                builder.push_str(" AS ");
                builder.push_identifier(&physical_table.name.name);

                if !row.is_empty() {
                    builder.push_str(" (");
                    builder.push_iter(row.iter(), ", ", |builder, (column_id, _)| {
                        builder.push_identifier(&column_id.get_column(database).name)
                    });
                    builder.push(')');
                }
            }
        }
    }
}

/// A value in a row of values. A parameter carries its type, but a null doesn't, so it is cast to
/// the column's type (otherwise, Postgres takes it as text and fails to compare it with the column).
fn build_row_value(
    column_id: ColumnId,
    value: &Column,
    database: &Database,
    builder: &mut SQLBuilder,
) {
    match value {
        Column::Null => {
            let column_type = &column_id.get_column(database).typ;
            let type_name = match column_type.as_any().downcast_ref::<EnumColumnType>() {
                Some(enum_type) => enum_type.enum_name.sql_name(),
                None => column_type.get_pg_type().name().to_string(),
            };
            builder.push_str("NULL::");
            builder.push_str(type_name);
        }
        _ => value.build(database, builder),
    }
}
