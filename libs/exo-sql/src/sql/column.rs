// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use maybe_owned::MaybeOwned;

use crate::{ColumnId, Database, ParamEquality, PhysicalColumnType, PhysicalTableName};

use super::{
    json_agg::JsonAgg, json_object::JsonObject, select::Select, transaction::TransactionStepId,
    ExpressionBuilder, SQLBuilder, SQLParamContainer,
};

/// A column-like concept covering any usage where a database table column could be used. For
/// example, in a predicate you can say `first_name = 'Sam'` or `first_name = last_name`. Here,
/// first_name, last_name, and `'Sam'` are serve as columns from our perspective. The variants
/// encode the exact semantics of each kind.
///
/// Essentially represents `<column>` in a `select <column>, <column> from <table>` or `<column> <>
/// <value>` in a predicate or `<column> = <value>` in an `update <table> set <column> = <value>`,
/// etc.
#[derive(Debug, PartialEq)]
pub enum Column {
    /// An actual physical column in a table
    Physical {
        column_id: ColumnId,
        table_alias: Option<String>,
    },
    /// A literal value such as a string or number e.g. 'Sam'. This will be mapped to a placeholder
    /// to avoid SQL injection.
    Param(SQLParamContainer),
    // An array parameter with a wrapping such as ANY() or ALL()
    ArrayParam {
        param: SQLParamContainer,
        wrapper: ArrayParamWrapper,
    },
    /// A JSON object. This is used to represent the result of a JSON object aggregation.
    JsonObject(JsonObject),
    /// A JSON array. This is used to represent the result of a JSON array aggregation.
    JsonAgg(JsonAgg),
    /// A sub-select query.
    SubSelect(Box<Select>),
    // TODO: Generalize the following to return any type of value, not just strings
    /// A constant string so that we can have a query return a particular value passed in as in
    /// `select 'Concert', id from "concerts"`. Here 'Concert' is the constant string. Needed to
    /// have a query return __typename set to a constant value
    Constant(String),
    /// All columns of a table. If the table is `None` should translate to `*`, else  `"table_name".*` or "schema"."table_name".*
    Star(Option<PhysicalTableName>),
    /// A null value
    Null,
    /// A function applied to a column. For example, `count(*)` or `lower(first_name)`.
    Function {
        function_name: String,
        column_id: ColumnId,
    },
}

#[derive(Debug, PartialEq)]
pub enum ArrayParamWrapper {
    Any,
    All,
    None,
}

impl Column {
    pub fn physical(column_id: ColumnId, table_alias: Option<String>) -> Self {
        Self::Physical {
            column_id,
            table_alias,
        }
    }
}

impl ExpressionBuilder for Column {
    fn build(&self, database: &Database, builder: &mut SQLBuilder) {
        match self {
            Column::Physical {
                column_id,
                table_alias,
            } => {
                let column = column_id.get_column(database);
                match table_alias {
                    Some(table_alias) => {
                        builder.push_column_with_table_alias(&column.name, table_alias);
                    }
                    _ => column.build(database, builder),
                }
            }
            Column::Function {
                function_name,
                column_id,
            } => {
                builder.push_str(function_name);
                builder.push('(');
                let column = column_id.get_column(database);
                column.build(database, builder);
                builder.push(')');
                if matches!(column.typ, PhysicalColumnType::Vector { .. })
                    && function_name != "count"
                {
                    // For vectors, we need to cast the result to a real array (otherwise it will be a string)
                    builder.push_str("::real[]");
                }
            }
            Column::Param(value) => builder.push_param(value.param()),
            Column::ArrayParam { param, wrapper } => {
                let wrapper_string = match wrapper {
                    ArrayParamWrapper::Any => "ANY",
                    ArrayParamWrapper::All => "ALL",
                    ArrayParamWrapper::None => "",
                };

                if wrapper_string.is_empty() {
                    builder.push_param(param.param());
                } else {
                    builder.push_str(wrapper_string);
                    builder.push('(');
                    builder.push_param(param.param());
                    builder.push(')');
                }
            }
            Column::JsonObject(obj) => {
                obj.build(database, builder);
            }
            Column::JsonAgg(agg) => agg.build(database, builder),
            Column::SubSelect(selection_table) => {
                builder.push('(');
                selection_table.build(database, builder);
                builder.push(')');
            }
            Column::Constant(value) => {
                builder.push('\'');
                builder.push_str(value);
                builder.push('\'');
            }
            Column::Star(table_name) => {
                if let Some(table_name) = table_name {
                    builder.push_table(table_name);
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
    Concrete(MaybeOwned<'a, Column>),
    // A template version of a column that will be replaced with a concrete column at runtime
    Template {
        col_index: usize,
        step_id: TransactionStepId,
    },
}

impl<'a> ParamEquality for ProxyColumn<'a> {
    fn param_eq(&self, other: &Self) -> Option<bool> {
        match (self, other) {
            (Self::Concrete(l), Self::Concrete(r)) => l.param_eq(r),
            _ => None,
        }
    }
}

impl<'a> PartialEq for ProxyColumn<'a> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Concrete(l), Self::Concrete(r)) => l == r,
            (
                Self::Template {
                    col_index: l_col_index,
                    step_id: l_step_id,
                },
                Self::Template {
                    col_index: r_col_index,
                    step_id: r_step_id,
                },
            ) => l_col_index == r_col_index && l_step_id == r_step_id,
            _ => false,
        }
    }
}
