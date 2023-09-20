// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use crate::Database;

use super::{
    physical_table::PhysicalTableName, select::Select, sql_operation::SQLOperation,
    ExpressionBuilder, SQLBuilder,
};

/// A query with common table expressions of the form `WITH <expressions> <select>`.
#[derive(Debug)]
pub struct WithQuery<'a> {
    /// The "WITH" expressions
    pub expressions: Vec<CteExpression<'a>>,
    /// The select statement
    pub select: Select,
}

/// A common table expression of the form `<name> AS (<operation>)`.
#[derive(Debug)]
pub struct CteExpression<'a> {
    /// The name of the expression
    pub name: String,
    /// The name of the table that this operation stands for. This allows us to substitute the table name in the select statement of `WithQuery`.
    pub table_name: Option<PhysicalTableName>,
    /// The SQL operation to be bound to the name
    pub operation: SQLOperation<'a>,
}

impl<'a> CteExpression<'a> {
    pub fn new_auto_name(table_name: &PhysicalTableName, operation: SQLOperation<'a>) -> Self {
        Self {
            name: table_name.synthetic_name(),
            table_name: Some(table_name.clone()),
            operation,
        }
    }
}

impl<'a> ExpressionBuilder for WithQuery<'a> {
    /// Build a CTE for the `WITH <expressions> <select>` syntax.
    fn build(&self, database: &Database, builder: &mut SQLBuilder) {
        builder.push_str("WITH ");
        builder.push_elems(database, &self.expressions, ", ");
        builder.push_space();

        let cte_table_map = self
            .expressions
            .iter()
            .flat_map(
                |CteExpression {
                     name, table_name, ..
                 }| {
                    table_name
                        .as_ref()
                        .map(|table_name| (table_name.clone(), name.clone()))
                },
            )
            .collect();
        builder.with_table_alias_map(cte_table_map, |builder| {
            self.select.build(database, builder);
        });
    }
}

impl ExpressionBuilder for CteExpression<'_> {
    /// Build a CTE expression for the `<name> AS (<operation>)` syntax.
    fn build(&self, database: &Database, builder: &mut SQLBuilder) {
        builder.push_identifier(&self.name);
        builder.push_str(" AS (");
        self.operation.build(database, builder);
        builder.push(')');
    }
}
