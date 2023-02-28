use maybe_owned::MaybeOwned;

use crate::PhysicalTable;

use super::{
    column::{Column, PhysicalColumn, ProxyColumn},
    transaction::{TransactionContext, TransactionStepId},
    Expression, ExpressionContext, ParameterBinding, SQLParamContainer,
};

#[derive(Debug)]
pub struct Insert<'a> {
    pub table: &'a PhysicalTable,
    pub column_names: Vec<&'a PhysicalColumn>,
    pub column_values_seq: Vec<Vec<MaybeOwned<'a, Column<'a>>>>,
    pub returning: Vec<MaybeOwned<'a, Column<'a>>>,
}

impl<'a> Expression for Insert<'a> {
    fn binding(&self, expression_context: &mut ExpressionContext) -> ParameterBinding {
        let table_binding = self.table.binding(expression_context);

        let (column_statements, col_params): (Vec<_>, Vec<_>) =
            expression_context.with_plain(|expression_context| {
                self.column_names
                    .iter()
                    .map(|column_names| column_names.binding(expression_context).tupled())
                    .unzip()
            });

        let (value_statements, value_params): (Vec<Vec<_>>, Vec<Vec<_>>) = self
            .column_values_seq
            .iter()
            .map(|column_values| {
                column_values
                    .iter()
                    .map(|value| value.binding(expression_context).tupled())
                    .unzip()
            })
            .unzip();

        let stmt = format!(
            "INSERT INTO {} ({}) VALUES {}",
            table_binding.stmt,
            column_statements.join(", "),
            value_statements
                .iter()
                .map(|v| format!("({})", v.join(", ")))
                .collect::<Vec<_>>()
                .join(", ")
        );

        let mut params = table_binding.params;
        params.extend(col_params.into_iter().flatten());
        params.extend(value_params.into_iter().flatten().into_iter().flatten());

        if self.returning.is_empty() {
            ParameterBinding { stmt, params }
        } else {
            let (ret_stmts, ret_params): (Vec<_>, Vec<_>) = self
                .returning
                .iter()
                .map(|ret| ret.binding(expression_context).tupled())
                .unzip();

            let stmt = format!("{} RETURNING {}", stmt, ret_stmts.join(", "));
            params.extend(ret_params.into_iter().flatten());

            ParameterBinding { stmt, params }
        }
    }
}

#[derive(Debug)]
pub struct TemplateInsert<'a> {
    pub table: &'a PhysicalTable,
    pub column_names: Vec<&'a PhysicalColumn>,
    pub column_values_seq: Vec<Vec<ProxyColumn<'a>>>,
    pub returning: Vec<MaybeOwned<'a, Column<'a>>>,
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
        column_values_seq: &'b [Vec<ProxyColumn<'b>>],
        row_index: usize,
        transaction_context: &TransactionContext,
    ) -> Vec<Vec<MaybeOwned<'b, Column<'b>>>> {
        column_values_seq
            .iter()
            .map(|row| {
                row.iter()
                    .map(|col| match col {
                        ProxyColumn::Concrete(col) => col.as_ref().into(),
                        ProxyColumn::Template { col_index, step_id } => MaybeOwned::Owned(
                            Column::Literal(MaybeOwned::Owned(SQLParamContainer::new(
                                transaction_context.resolve_value(*step_id, row_index, *col_index),
                            ))),
                        ),
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
                column_names,
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
                column_names: column_names.clone(),
                column_values_seq: resolved_cols,
                returning: returning.iter().map(|ret| ret.as_ref().into()).collect(),
            })
        }
    }
}
