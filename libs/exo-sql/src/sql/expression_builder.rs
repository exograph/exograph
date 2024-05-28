// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use maybe_owned::MaybeOwned;

use super::{sql_param::SQLParamWithType, SQLBuilder};
use crate::Database;

/// A trait for types that can build themselves into an SQL expression.
///
/// Each constituent of an SQL expression (column, table, function, select, etc.) should implement
/// this trait, which can then be used to hierarchically build an SQL string and the list of
/// parameters to be supplied to it.
pub trait ExpressionBuilder {
    /// Build the SQL expression into the given SQL builder
    fn build(&self, database: &Database, builder: &mut SQLBuilder);

    /// Build the SQL expression into a string and return it This is useful for testing/debugging, where we
    /// want to assert on the generated SQL without going through the whole process of creating an
    /// SQLBuilder, then building the SQL expression into it, and finally extracting the SQL string
    /// and params.
    fn to_sql(&self, database: &Database) -> (String, Vec<SQLParamWithType>)
    where
        Self: Sized,
    {
        let mut builder = SQLBuilder::new();
        self.build(database, &mut builder);
        builder.into_sql()
    }
}

impl<T> ExpressionBuilder for Box<T>
where
    T: ExpressionBuilder,
{
    fn build(&self, database: &Database, builder: &mut SQLBuilder) {
        self.as_ref().build(database, builder)
    }
}

impl<'a, T> ExpressionBuilder for MaybeOwned<'a, T>
where
    T: ExpressionBuilder,
{
    fn build(&self, database: &Database, builder: &mut SQLBuilder) {
        self.as_ref().build(database, builder)
    }
}

impl<T> ExpressionBuilder for &T
where
    T: ExpressionBuilder,
{
    fn build(&self, database: &Database, builder: &mut SQLBuilder) {
        (**self).build(database, builder)
    }
}
