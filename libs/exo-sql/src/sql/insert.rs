// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use maybe_owned::MaybeOwned;

use crate::{Database, PhysicalTable};

use super::{
    ExpressionBuilder, SQLBuilder, SQLParamContainer,
    column::{Column, ProxyColumn},
    physical_column::PhysicalColumn,
    transaction::{TransactionContext, TransactionStepId},
};

/// An insert operation.
#[derive(Debug)]
pub struct Insert<'a> {
    /// The table to insert into.
    pub table: &'a PhysicalTable,
    /// The columns to insert into such as `(age, name)`
    pub columns: Vec<&'a PhysicalColumn>,
    /// The values to insert such as `(30, "John"), (35, "Jane")`
    pub values_seq: Vec<Vec<MaybeOwned<'a, Column>>>,
    /// The columns to return.
    pub returning: Vec<MaybeOwned<'a, Column>>,
}

impl ExpressionBuilder for Insert<'_> {
    /// Build the insert statement for the form `INSERT INTO <table> (<columns>) VALUES (<values>)
    /// RETURNING <returning-columns>`. The `RETURNING` clause is omitted if the list of columns to
    /// return is empty.
    fn build(&self, database: &Database, builder: &mut SQLBuilder) {
        builder.push_str("INSERT INTO ");
        self.table.build(database, builder);

        if self.columns.is_empty() {
            // If none of the columns have been provided, we can use DEFAULT VALUES.
            // This can happen if all fields of a type have a default value and no explicit values are provided.
            builder.push_str(" DEFAULT VALUES");
        } else {
            builder.push_str(" (");
            builder.without_fully_qualified_column_names(|builder| {
                builder.push_elems(database, &self.columns, ", ");
            });

            builder.push_str(") VALUES (");

            builder.push_iter(self.values_seq.iter(), "), (", |builder, values| {
                builder.push_elems(database, values, ", ");
            });
            builder.push(')');
        }

        if !self.returning.is_empty() {
            builder.push_str(" RETURNING ");
            builder.push_elems(database, &self.returning, ", ")
        }
    }
}

#[derive(Debug)]
pub struct TemplateInsert<'a> {
    pub table: &'a PhysicalTable,
    pub columns: Vec<&'a PhysicalColumn>,
    pub column_values_seq: Vec<Vec<ProxyColumn<'a>>>,
    pub returning: Vec<Column>,
}

impl<'a> TemplateInsert<'a> {
    fn has_template_columns(&self) -> bool {
        self.column_values_seq.iter().any(|column_values| {
            column_values
                .iter()
                .any(|value| matches!(value, ProxyColumn::Template { .. }))
        })
    }

    fn expand_row<'b>(
        column_values_seq: &'b [Vec<ProxyColumn>],
        row_index: usize,
        transaction_context: &TransactionContext,
    ) -> Vec<Vec<MaybeOwned<'b, Column>>> {
        column_values_seq
            .iter()
            .map(|row| {
                row.iter()
                    .map(|col| match col {
                        ProxyColumn::Concrete(col) => col.as_ref().into(),
                        ProxyColumn::Template { col_index, step_id } => {
                            MaybeOwned::Owned(Column::Param(SQLParamContainer::from_sql_value(
                                transaction_context.resolve_value(*step_id, row_index, *col_index),
                            )))
                        }
                    })
                    .collect()
            })
            .collect()
    }

    pub fn resolve(
        &'a self,
        prev_step_id: TransactionStepId,
        transaction_context: &TransactionContext,
    ) -> Option<Insert<'a>> {
        let row_count = transaction_context.row_count(prev_step_id);

        // If there are template columns, but no way to resolve them, this operation need not be performed
        // For example, if we are updating concert_artists while updating concerts, and there are no matching concerts
        // (determined by the where param to updateConcerts), then we don't need to update the concert_artists
        if self.has_template_columns() && row_count == 0 {
            None
        } else {
            let TemplateInsert {
                table,
                columns,
                column_values_seq,
                returning,
            } = self;

            let resolved_cols = (0..row_count)
                .flat_map(move |row_index| {
                    Self::expand_row(column_values_seq, row_index, transaction_context)
                })
                .collect();

            Some(Insert {
                table,
                columns: columns.clone(),
                values_seq: resolved_cols,
                returning: returning.iter().map(|ret| ret.into()).collect(),
            })
        }
    }
}
