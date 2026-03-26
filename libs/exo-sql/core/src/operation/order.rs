// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use crate::{ColumnId, Ordering};

use super::DatabaseExtension;

#[derive(Debug, PartialEq, Clone)]
pub struct OrderByElement<Ext: DatabaseExtension>(
    pub OrderByElementExpr<Ext>,
    pub Ordering,
    pub Option<String>,
);

#[derive(Debug, PartialEq, Clone)]
pub enum OrderByElementExpr<Ext: DatabaseExtension> {
    Column(ColumnId),
    /// Database-specific ordering expression (e.g., pgvector distance)
    Extension(Ext::OrderByExtension),
}

#[derive(Debug, PartialEq, Clone)]
pub struct OrderBy<Ext: DatabaseExtension>(pub Vec<OrderByElement<Ext>>);

impl<Ext: DatabaseExtension> OrderBy<Ext> {
    pub fn new(elements: Vec<OrderByElement<Ext>>) -> Self {
        Self(elements)
    }
}

impl<Ext: DatabaseExtension> OrderByElement<Ext> {
    pub fn new(column_id: ColumnId, ordering: Ordering, table_alias: Option<String>) -> Self {
        Self(OrderByElementExpr::Column(column_id), ordering, table_alias)
    }

    pub fn from_expr(
        expr: OrderByElementExpr<Ext>,
        ordering: Ordering,
        table_alias: Option<String>,
    ) -> Self {
        Self(expr, ordering, table_alias)
    }
}
