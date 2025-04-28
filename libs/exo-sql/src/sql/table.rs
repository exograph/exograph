// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use crate::{Database, TableId};

use super::{
    join::LeftJoin, schema_object::SchemaObjectName, select::Select, ExpressionBuilder, SQLBuilder,
};

/// A table-like concept that can be used in in place of `SELECT FROM <table-query> ...`.
#[derive(Debug, PartialEq)]
pub enum Table {
    /// A physical table such as `concerts`.
    Physical {
        table_id: TableId,
        alias: Option<String>,
    },
    /// A join between two tables such as `concerts LEFT JOIN venues ON concerts.venue_id = venues.id`.
    Join(LeftJoin),
    /// A sub-select such as `(SELECT * FROM concerts) AS concerts`.
    SubSelect {
        select: Box<Select>,
        /// The alias of the sub-select (optional, since we need to alias the sub-select when used in a FROM clause)
        alias: Option<(String, SchemaObjectName)>,
    },
}

impl Table {
    pub fn physical(table_id: TableId, alias: Option<String>) -> Self {
        Table::Physical { table_id, alias }
    }
}

impl ExpressionBuilder for Table {
    /// Build the table into a SQL string.
    fn build(&self, database: &Database, builder: &mut SQLBuilder) {
        match self {
            Table::Physical { table_id, alias } => {
                let physical_table = database.get_table(*table_id);
                physical_table.build(database, builder);

                if let Some(alias) = alias {
                    // If the the table name is the same as the alias (and the table is in the "public" schema), we don't need to alias it
                    // This avoid unnecessary aliasing like `SELECT * FROM concerts AS concerts`
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
