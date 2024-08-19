// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use crate::{
    asql::order_by::AbstractOrderByExpr,
    sql::order::{OrderBy, OrderByElement, OrderByElementExpr, VectorDistanceOperand},
    transform::transformer::OrderByTransformer,
    AbstractOrderBy, ColumnPath, Database,
};

use super::{selection_level::SelectionLevel, Postgres};

impl OrderByTransformer for Postgres {
    /// Transforms an abstract order-by clause into a concrete one
    /// by replacing the abstract column paths with physical ones,
    /// which will be used to generate the SQL query like:
    ///
    /// ```sql
    /// ORDER BY table.column ASC, table2.column2 DESC
    /// ```
    fn to_order_by<'a>(
        &self,
        order_by: &AbstractOrderBy,
        selection_level: &SelectionLevel,
        database: &Database,
    ) -> OrderBy {
        OrderBy(
            order_by
                .0
                .iter()
                .map(|(expr, ordering)| match expr {
                    AbstractOrderByExpr::Column(path) => {
                        let table_alias = match (selection_level.prefix(database), path.alias()) {
                            (Some(prefix), Some(alias)) => Some(format!("{}${}", prefix, alias)),
                            (None, Some(alias)) => Some(alias),
                            _ => None,
                        };

                        let column_id = path.leaf_column();
                        OrderByElement::new(column_id, *ordering, table_alias)
                    }
                    AbstractOrderByExpr::VectorDistance(lhs, rhs, op) => {
                        fn to_column(column_path: &ColumnPath) -> VectorDistanceOperand {
                            match column_path {
                                ColumnPath::Physical(path) => {
                                    VectorDistanceOperand::PhysicalColumn(path.leaf_column())
                                }
                                ColumnPath::Param(value) => {
                                    VectorDistanceOperand::Param(value.clone())
                                }
                                _ => panic!("Expected physical column path or a parameter"),
                            }
                        }
                        let lhs_column = to_column(lhs);
                        let rhs_column = to_column(rhs);
                        let expr = OrderByElementExpr::VectorDistance(lhs_column, rhs_column, *op);

                        OrderByElement(expr, *ordering, None)
                    }
                })
                .collect(),
        )
    }
}
