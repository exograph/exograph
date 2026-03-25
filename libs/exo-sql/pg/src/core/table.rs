// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use exo_sql_core::Database;

use crate::{ExpressionBuilder, SQLBuilder};

use crate::core::pg_extension::PgExtension;

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
        }
    }
}
