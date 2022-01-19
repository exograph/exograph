use std::rc::Rc;

use maybe_owned::MaybeOwned;

use super::{
    column::{Column, PhysicalColumn, ProxyColumn},
    predicate::Predicate,
    transaction::TransactionStep,
    Expression, ExpressionContext, ParameterBinding, TableQuery,
};

#[derive(Debug)]
pub struct Update<'a> {
    pub table: Rc<TableQuery<'a>>,
    pub predicate: Rc<MaybeOwned<'a, Predicate<'a>>>,
    pub column_values: Vec<(&'a PhysicalColumn, Rc<MaybeOwned<'a, Column<'a>>>)>,
    pub returning: Rc<Vec<MaybeOwned<'a, Column<'a>>>>,
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

/// An update whose columns may refer to an earlier sql expression through a proxy column.
#[derive(Debug)]
pub struct TemplateUpdate<'a> {
    pub table: TableQuery<'a>,
    pub predicate: Predicate<'a>,
    pub column_values: Vec<(&'a PhysicalColumn, ProxyColumn<'a>)>,
    pub returning: Vec<MaybeOwned<'a, Column<'a>>>,
}

impl<'a> TemplateUpdate<'a> {
    /// Resolve a template update to update expressions.
    /// The prev_step will govern how many update expressions this method will return. Use use the prev_step to
    /// resolve the proxy columns.
    pub fn resolve(self, prev_step: Rc<TransactionStep<'a>>) -> Vec<Update<'a>> {
        let rows = prev_step.resolved_value().borrow().len();

        let TemplateUpdate {
            table,
            predicate,
            column_values,
            returning,
        } = self;

        let table = Rc::new(table);
        let predicate: Rc<MaybeOwned<Predicate>> = Rc::new(predicate.into());
        let returning = Rc::new(returning);

        (0..rows)
            .map(|row_index| {
                let resolved_column_values = column_values
                    .clone()
                    .iter()
                    .map(|(physical_col, col)| {
                        let resolved_col = match col {
                            ProxyColumn::Concrete(col) => col.clone(),
                            ProxyColumn::Template { col_index, step } => {
                                Rc::new(MaybeOwned::Owned(Column::Lazy {
                                    row_index,
                                    col_index: *col_index,
                                    step: step.clone(),
                                }))
                            }
                        };
                        (*physical_col, resolved_col)
                    })
                    .collect();

                Update {
                    table: table.clone(),
                    predicate: predicate.clone(),
                    column_values: resolved_column_values,
                    returning: returning.clone(),
                }
            })
            .collect()
    }
}
