use super::{
    column::Column, predicate::Predicate, Expression, ExpressionContext, ParameterBinding,
    PhysicalTable,
};

pub struct Update<'a> {
    pub table: &'a PhysicalTable,
    pub predicate: &'a Predicate<'a>,
    pub column_values: Vec<(&'a Column<'a>, &'a Column<'a>)>,
    pub returning: Vec<&'a Column<'a>>,
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
