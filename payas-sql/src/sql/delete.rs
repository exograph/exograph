use std::rc::Rc;

use maybe_owned::MaybeOwned;

use super::{
    column::Column, physical_table::PhysicalTable, predicate::Predicate,
    transaction::TransactionStep, Expression, ExpressionContext, ParameterBinding,
};

#[derive(Debug)]
pub struct Delete<'a> {
    pub table: &'a PhysicalTable,
    pub predicate: Option<MaybeOwned<'a, Predicate<'a>>>,
    pub returning: Vec<&'a Column<'a>>,
}

impl<'a> Expression for Delete<'a> {
    fn binding(&self, expression_context: &mut ExpressionContext) -> ParameterBinding {
        let table_binding = self.table.binding(expression_context);

        let predicate_binding = self
            .predicate
            .as_ref()
            .map(|predicate| predicate.binding(expression_context));

        let (stmt, mut params) = match predicate_binding {
            Some(predicate_binding) => {
                let mut params = table_binding.params;
                params.extend(predicate_binding.params);

                (
                    format!(
                        "DELETE FROM {} WHERE {}",
                        table_binding.stmt, predicate_binding.stmt
                    ),
                    params,
                )
            }
            None => (
                format!("DELETE FROM {}", table_binding.stmt,),
                table_binding.params,
            ),
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
    pub table: &'a PhysicalTable,
    pub predicate: Option<&'a Predicate<'a>>,
    pub returning: Vec<&'a Column<'a>>,
}

// TODO: Tie this properly to the prev_step
impl<'a> TemplateDelete<'a> {
    pub fn resolve(&'a self, _prev_step: Rc<TransactionStep<'a>>) -> Delete<'a> {
        Delete {
            table: self.table,
            predicate: self.predicate.map(|p| p.into()),
            returning: self.returning.clone(),
        }
    }
}
