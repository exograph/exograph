use super::{
    column::Column, physical_table::PhysicalTable, predicate::Predicate, Expression,
    ExpressionContext, ParameterBinding,
};

#[derive(Debug, Clone)]
pub struct Delete<'a> {
    pub underlying: &'a PhysicalTable,
    pub predicate: Option<&'a Predicate<'a>>,
    pub returning: Vec<&'a Column<'a>>,
}

impl<'a> Expression for Delete<'a> {
    fn binding(&self, expression_context: &mut ExpressionContext) -> ParameterBinding {
        let table_binding = self.underlying.binding(expression_context);

        let predicate_binding = self
            .predicate
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
