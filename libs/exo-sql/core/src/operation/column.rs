// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use crate::{ColumnId, SchemaObjectName};

use super::DatabaseExtension;
use super::ParamEquality;
use super::function::Function;
use super::predicate::ColumnPredicate;
use super::select::Select;

/// A column-like concept covering any usage where a database table column could be used.
///
/// The `Ext` type parameter allows database-specific extensions (e.g., Postgres-specific
/// parameter binding, JSON aggregation, etc.) without polluting the core types.
#[derive(Debug, PartialEq, Clone)]
pub enum Column<Ext: DatabaseExtension> {
    /// An actual physical column in a table
    Physical {
        column_id: ColumnId,
        table_alias: Option<String>,
    },
    /// A column that is an array of columns (used for IN clauses)
    ColumnArray(Vec<Column<Ext>>),
    /// A sub-select query
    SubSelect(Box<Select<Ext>>),
    // TODO: Generalize the following to return any type of value, not just strings
    /// A constant string so that we can have a query return a particular value passed in as in
    /// `select 'Concert', id from "concerts"`. Here 'Concert' is the constant string. Needed to
    /// have a query return __typename set to a constant value
    Constant(String),
    /// All columns of a table (`*` or `"table".*`)
    Star(Option<SchemaObjectName>),
    /// A null value
    Null,
    /// A literal value such as a string or number. Mapped to a placeholder to avoid SQL injection.
    Param(Ext::Param),
    /// A function applied to a column (e.g., `count(id)`)
    Function(Function<Ext>),
    /// A predicate used as a column expression
    Predicate(Box<ColumnPredicate<Ext>>),
    /// Database-specific extension
    Extension(Ext::ColumnExtension),
}

impl<Ext: DatabaseExtension> Column<Ext> {
    pub fn physical(column_id: ColumnId, table_alias: Option<String>) -> Self {
        Self::Physical {
            column_id,
            table_alias,
        }
    }
}

impl<Ext: DatabaseExtension> ParamEquality for Column<Ext> {
    fn param_eq(&self, other: &Self) -> Option<bool> {
        match (self, other) {
            (Column::Param(v1), Column::Param(v2)) => Some(v1 == v2),
            (Column::Extension(e1), Column::Extension(e2)) => e1.param_eq(e2),
            _ => None,
        }
    }
}
