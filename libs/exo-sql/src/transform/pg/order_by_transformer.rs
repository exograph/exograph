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
    AbstractOrderBy,
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
    fn to_order_by<'a>(&self, order_by: &AbstractOrderBy) -> OrderBy {
        OrderBy(
            order_by
                .0
                .iter()
                .map(|(path, ordering)| {
                    let (column_id, table_alias) = (path.leaf_column(), path.alias());
                    OrderByElement::new(column_id, *ordering, table_alias)
                })
                .collect(),
        )
    }
}
