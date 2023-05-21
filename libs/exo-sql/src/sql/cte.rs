// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use crate::Database;

use super::{select::Select, sql_operation::SQLOperation, ExpressionBuilder, SQLBuilder};

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
    /// The SQL operation to be bound to the name
    pub operation: SQLOperation<'a>,
}

impl<'a> ExpressionBuilder for WithQuery<'a> {
    /// Build a CTE for the `WITH <expressions> <select>` syntax.
    fn build(&self, database: &Database, builder: &mut SQLBuilder) {
        builder.push_str("WITH ");
        builder.push_elems(database, &self.expressions, ", ");
        builder.push_space();
        self.select.build(database, builder);
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
