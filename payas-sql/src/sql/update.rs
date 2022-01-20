use maybe_owned::MaybeOwned;

use super::{
    column::{Column, PhysicalColumn, ProxyColumn},
    predicate::Predicate,
    transaction::{TransactionContext, TransactionStepId},
    Expression, ExpressionContext, ParameterBinding, PhysicalTable,
};

#[derive(Debug)]
pub struct Update<'a> {
    pub table: &'a PhysicalTable,
    pub predicate: MaybeOwned<'a, Predicate<'a>>,
    pub column_values: Vec<(&'a PhysicalColumn, MaybeOwned<'a, Column<'a>>)>,
    pub returning: Vec<MaybeOwned<'a, Column<'a>>>,
}

impl<'a> Expression for Update<'a> {
    fn binding(&self, expression_context: &mut ExpressionContext) -> ParameterBinding {
        let table_binding = self.table.binding(expression_context);

        let (col_stmts, col_params): (Vec<_>, Vec<_>) =
            expression_context.with_plain(|expression_context| {
                self.column_values
                    .iter()
                    .map(|(column, value)| {
                        let col_binding = column.binding(expression_context);
                        let value_binding = value.binding(expression_context);

                        let mut params = col_binding.params;
                        params.extend(value_binding.params);
                        (
                            format!("{} = {}", col_binding.stmt, value_binding.stmt),
                            params,
                        )
                    })
                    .unzip()
            });

        let predicate_binding = self.predicate.binding(expression_context);

        let stmt = format!(
            "UPDATE {} SET {} WHERE {}",
            table_binding.stmt,
            col_stmts.join(", "),
            predicate_binding.stmt
        );

        let mut params = table_binding.params;
        params.extend(col_params.into_iter().flatten());
        params.extend(predicate_binding.params.into_iter());

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
pub struct TemplateUpdate<'a> {
    pub table: &'a PhysicalTable,
    pub predicate: Predicate<'a>,
    pub column_values: Vec<(&'a PhysicalColumn, ProxyColumn<'a>)>,
    pub returning: Vec<MaybeOwned<'a, Column<'a>>>,
}

impl<'a> TemplateUpdate<'a> {
    pub fn resolve(
        &'a self,
        prev_step_id: TransactionStepId,
        transaction_context: &TransactionContext,
    ) -> Vec<Update<'a>> {
        let rows = transaction_context.row_count(prev_step_id);

        let TemplateUpdate {
            table,
            predicate,
            column_values,
            returning,
        } = self;

        (0..rows)
            .map(|row_index| {
                let resolved_column_values = column_values
                    .iter()
                    .map(|(physical_col, col)| {
                        let resolved_col = match col {
                            ProxyColumn::Concrete(col) => col.as_ref().into(),
                            ProxyColumn::Template { col_index, step_id } => {
                                MaybeOwned::Owned(Column::Literal(Box::new(
                                    transaction_context
                                        .resolve_value(*step_id, row_index, *col_index),
                                )))
                            }
                        };
                        (*physical_col, resolved_col)
                    })
                    .collect();
                Update {
                    table,
                    predicate: predicate.into(),
                    column_values: resolved_column_values,
                    returning: returning.iter().map(|col| col.as_ref().into()).collect(),
                }
            })
            .collect()
    }
}
