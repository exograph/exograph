use std::rc::Rc;

use maybe_owned::MaybeOwned;

use super::{
    column::Column, physical_table::PhysicalTable, predicate::Predicate,
    transaction::TransactionStep, Expression, ExpressionContext, ParameterBinding, TableQuery,
};

#[derive(Debug)]
pub struct Delete<'a> {
    pub table: TableQuery<'a>,
    pub predicate: Rc<MaybeOwned<'a, Predicate<'a>>>,
    pub returning: Rc<Vec<MaybeOwned<'a, Column<'a>>>>,
}

impl<'a> Expression for Delete<'a> {
    fn binding(&self, expression_context: &mut ExpressionContext) -> ParameterBinding {
        let table_binding = self.table.binding(expression_context);

        let predicate_binding = self.predicate.binding(expression_context);

        let (stmt, mut params) = {
            let mut params = table_binding.params;
            params.extend(predicate_binding.params);

            (
                format!(
                    "DELETE FROM {} WHERE {}",
                    table_binding.stmt, predicate_binding.stmt
                ),
                params,
            )
        };

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
pub struct TemplateDelete<'a> {
    pub table: TableQuery<'a>,
    pub predicate: Predicate<'a>,
    pub returning: Vec<MaybeOwned<'a, Column<'a>>>,
}

// TODO: Tie this properly to the prev_step
impl<'a> TemplateDelete<'a> {
    pub fn resolve(self, _prev_step: Rc<TransactionStep<'a>>) -> Delete<'a> {
        let TemplateDelete {
            table,
            predicate,
            returning,
        } = self;

        Delete {
            table,
            predicate: Rc::new(predicate.into()),
            returning: Rc::new(returning),
        }
    }
}
