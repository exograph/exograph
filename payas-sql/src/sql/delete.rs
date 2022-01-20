use maybe_owned::MaybeOwned;

use super::{
    column::Column, physical_table::PhysicalTable, predicate::Predicate, Expression,
    ExpressionContext, ParameterBinding,
};

#[derive(Debug)]
pub struct Delete<'a> {
    pub table: &'a PhysicalTable,
    pub predicate: MaybeOwned<'a, Predicate<'a>>,
    pub returning: Vec<MaybeOwned<'a, Column<'a>>>,
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
    pub table: &'a PhysicalTable,
    pub predicate: Predicate<'a>,
    pub returning: Vec<MaybeOwned<'a, Column<'a>>>,
}

// TODO: Tie this properly to the prev_step
impl<'a> TemplateDelete<'a> {
    pub fn resolve(&'a self) -> Delete<'a> {
        let TemplateDelete {
            table,
            predicate,
            returning,
        } = self;

        Delete {
            table,
            predicate: predicate.into(),
            returning: returning
                .iter()
                .map(|c| MaybeOwned::Borrowed(c.as_ref()))
                .collect(),
        }
    }
}
