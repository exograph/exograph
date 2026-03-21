// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use maybe_owned::MaybeOwned;

use crate::ParamEquality;
use exo_sql_core::Database;

use crate::pg_extension::PgExtension;

use crate::{ExpressionBuilder, SQLBuilder, transaction::TransactionStepId};

// Re-export the core Column type specialized to PgExtension
pub type Column = exo_sql_core::operation::Column<PgExtension>;

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
            Column::ColumnArray(columns) => {
                if columns.len() > 1 {
                    builder.push('(');
                }
                builder.push_elems(database, columns, ",");
                if columns.len() > 1 {
                    builder.push(')');
                }
            }
            Column::Function(function) => {
                function.build(database, builder);
            }
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
            Column::Predicate(predicate) => {
                builder.push('(');
                predicate.build(database, builder);
                builder.push(')');
            }
            Column::Extension(ext) => {
                ext.build(database, builder);
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

impl ParamEquality for ProxyColumn<'_> {
    fn param_eq(&self, other: &Self) -> Option<bool> {
        match (self, other) {
            (Self::Concrete(l), Self::Concrete(r)) => l.param_eq(r),
            _ => None,
        }
    }
}

impl PartialEq for ProxyColumn<'_> {
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

// ParamEquality for Column<PgExtension> is provided by core's blanket impl
// which delegates to PgExtension::param_eq (defined in pg_extension.rs)
