use super::{
    select::Select, sql_operation::SQLOperation, Expression, ExpressionContext, ParameterBinding,
};

pub struct Cte<'a> {
    ctes: Vec<(String, SQLOperation<'a>)>,
    select: Select<'a>,
}

impl<'a> Expression for Cte<'a> {
    fn binding(&self, expression_context: &mut ExpressionContext) -> ParameterBinding {
        let (cte_statements, cte_params): (Vec<_>, Vec<_>) = self
            .ctes
            .iter()
            .map(|(name, operation)| {
                let ParameterBinding { stmt, params } = operation.binding(expression_context);
                (format!("{} AS ({})", name, stmt), params)
            })
            .unzip();

        let select_binding = self.select.binding(expression_context);

        let stmt = format!("WITH {} {}", cte_statements.join(", "), select_binding.stmt);

        let mut params: Vec<_> = cte_params.into_iter().flatten().collect();
        params.extend(select_binding.params.iter());

        ParameterBinding { stmt, params }
    }
}
