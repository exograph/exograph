use std::rc::Rc;

use maybe_owned::MaybeOwned;

use super::{
    column::{Column, PhysicalColumn, ProxyColumn},
    transaction::TransactionStep,
    Expression, ExpressionContext, ParameterBinding, PhysicalTable,
};

#[derive(Debug)]
pub struct Insert<'a> {
    pub table: &'a PhysicalTable,
    pub column_names: Vec<&'a PhysicalColumn>,
    pub column_values_seq: Vec<Vec<Rc<MaybeOwned<'a, Column<'a>>>>>,
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

    fn resolve_row<'b>(
        column_values_seq: Vec<Vec<ProxyColumn<'b>>>,
        row_index: usize,
    ) -> Vec<Vec<Rc<MaybeOwned<'b, Column<'b>>>>> {
        column_values_seq
            .into_iter()
            .map(|row| {
                row.into_iter()
                    .map(|col| match col {
                        ProxyColumn::Concrete(col) => col.clone().into(),
                        ProxyColumn::Template { col_index, step } => {
                            Rc::new(MaybeOwned::Owned(Column::Lazy {
                                row_index,
                                col_index,
                                step,
                            }))
                        }
                    })
                    .collect()
            })
            .collect()
    }

    pub fn resolve(self, prev_step: Rc<TransactionStep<'a>>) -> Option<Insert<'a>> {
        let row_count = prev_step.resolved_value().borrow().len();

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
                .flat_map(move |row_index| Self::resolve_row(column_values_seq.clone(), row_index))
                .collect();

            Some(Insert {
                table,
                column_names: column_names.clone(),
                column_values_seq: resolved_cols,
                returning,
            })
        }
    }
}
