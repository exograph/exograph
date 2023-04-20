// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use super::{
    json_agg::JsonAgg, json_object::JsonObject, physical_column::PhysicalColumn, select::Select,
    transaction::TransactionStepId, ExpressionBuilder, SQLBuilder, SQLParamContainer,
};
use maybe_owned::MaybeOwned;

/// A column-like concept covering any usage where a database table column could be used. For
/// example, in a predicate you can say `first_name = 'Sam'` or `first_name = last_name`. Here,
/// first_name, last_name, and `'Sam'` are serve as columns from our perspective. The variants
/// encode the exact semantics of each kind.
///
/// Essentially represents `<column>` in a `select <column>, <column> from <table>` or `<column> <>
/// <value>` in a predicate or `<column> = <value>` in an `update <table> set <column> = <value>`,
/// etc.
#[derive(Debug, PartialEq)]
pub enum Column<'a> {
    /// An actual physical column in a table
    Physical(&'a PhysicalColumn),
    /// A literal value such as a string or number e.g. 'Sam'. This will be mapped to a placeholder
    /// to avoid SQL injection.
    Param(SQLParamContainer),
    /// A JSON object. This is used to represent the result of a JSON object aggregation.
    JsonObject(JsonObject<'a>),
    /// A JSON array. This is used to represent the result of a JSON array aggregation.
    JsonAgg(JsonAgg<'a>),
    /// A sub-select query.
    SubSelect(Box<Select<'a>>),
    // TODO: Generalize the following to return any type of value, not just strings
    /// A constant string so that we can have a query return a particular value passed in as in
    /// `select 'Concert', id from "concerts"`. Here 'Concert' is the constant string. Needed to
    /// have a query return __typename set to a constant value
    Constant(String),
    /// All columns of a table. If the table is `None` should translate to `*`, else  `"table_name".*`
    Star(Option<String>),
    /// A null value
    Null,
    /// A function applied to a column. For example, `count(*)` or `lower(first_name)`.
    Function {
        function_name: String,
        column: &'a PhysicalColumn,
    },
}

impl<'a> ExpressionBuilder for Column<'a> {
    fn build(&self, builder: &mut SQLBuilder) {
        match self {
            Column::Physical(pc) => pc.build(builder),
            Column::Function {
                function_name,
                column,
            } => {
                builder.push_str(function_name);
                builder.push('(');
                column.build(builder);
                builder.push(')');
            }
            Column::Param(value) => builder.push_param(value.param()),
            Column::JsonObject(obj) => {
                obj.build(builder);
            }
            Column::JsonAgg(agg) => agg.build(builder),
            Column::SubSelect(selection_table) => {
                builder.push('(');
                selection_table.build(builder);
                builder.push(')');
            }
            Column::Constant(value) => {
                builder.push('\'');
                builder.push_str(value);
                builder.push('\'');
            }
            Column::Star(table_name) => {
                if let Some(table_name) = table_name {
                    builder.push_identifier(table_name);
                    builder.push('.');
                }
                builder.push('*');
            }
            Column::Null => {
                builder.push_str("NULL");
            }
        }
    }
}

/// A column bound to a particular transaction step. This is used to represent a column in a
/// multi-step insert/update.
#[derive(Debug)]
pub enum ProxyColumn<'a> {
    Concrete(MaybeOwned<'a, Column<'a>>),
    // A template version of a column that will be replaced with a concrete column at runtime
    Template {
        col_index: usize,
        step_id: TransactionStepId,
    },
}
