// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use crate::{Database, TableId};

use super::{join::LeftJoin, select::Select, ExpressionBuilder, SQLBuilder};

/// A table-like concept that can be used in in place of `SELECT FROM <table-query> ...`.
#[derive(Debug, PartialEq)]
pub enum Table<'a> {
    /// A physical table such as `concerts`.
    Physical(TableId),
    /// A join between two tables such as `concerts LEFT JOIN venues ON concerts.venue_id = venues.id`.
    Join(LeftJoin<'a>),
    /// A sub-select such as `(SELECT * FROM concerts) AS concerts`.
    SubSelect {
        select: Box<Select<'a>>,
        /// The alias of the sub-select (optional, since we need to alias the sub-select when used in a FROM clause)
        alias: Option<String>,
    },
}

impl<'a> ExpressionBuilder for Table<'a> {
    /// Build the table into a SQL string.
    fn build(&self, database: &Database, builder: &mut SQLBuilder) {
        match self {
            Table::Physical(physical_table) => {
                builder.push_identifier(&database.get_table(*physical_table).name)
            }
            Table::Join(join) => join.build(database, builder),
            Table::SubSelect { select, alias } => {
                builder.push('(');
                select.build(database, builder);
                builder.push(')');
                if let Some(alias) = alias {
                    builder.push_str(" AS ");
                    builder.push_identifier(alias);
                }
            }
        }
    }
}
