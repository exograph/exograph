// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use crate::{
    sql::order::{OrderBy, OrderByElement},
    transform::transformer::OrderByTransformer,
    AbstractOrderBy, ColumnPath, PhysicalColumn,
};

use super::Postgres;

impl OrderByTransformer for Postgres {
    /// Transforms an abstract order-by clause into a concrete one
    /// by replacing the abstract column paths with physical ones,
    /// which will be used to generate the SQL query like:
    ///
    /// ```sql
    /// ORDER BY table.column ASC, table2.column2 DESC
    /// ```
    fn to_order_by<'a>(&self, order_by: &AbstractOrderBy<'a>) -> OrderBy<'a> {
        OrderBy(
            order_by
                .0
                .iter()
                .map(|(path, ordering)| OrderByElement::new(leaf_column(path), *ordering))
                .collect(),
        )
    }
}

fn leaf_column<'a>(column_path: &ColumnPath<'a>) -> &'a PhysicalColumn {
    match column_path {
        ColumnPath::Physical(links) => links.last().unwrap().self_column.0,
        _ => panic!("Cannot get leaf column from literal or null"),
    }
}
