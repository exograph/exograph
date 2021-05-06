use super::{column::Column, Expression, ExpressionContext, ParameterBinding, PhysicalTable};

pub struct Insert<'a> {
    pub underlying: &'a PhysicalTable,
    pub column_values: Vec<(&'a Column<'a>, &'a Column<'a>)>,
    pub returning: Vec<&'a Column<'a>>,
}

impl<'a> Expression for Insert<'a> {
    fn binding(&self, expression_context: &mut ExpressionContext) -> ParameterBinding {
        let table_binding = self.underlying.binding(expression_context);

        let (col_bindings, value_bindings): (Vec<_>, Vec<_>) = self
            .column_values
            .iter()
            .map(|(column, value)| {
                let col_binding = column.binding(expression_context);
                let value_binding = value.binding(expression_context);
                (col_binding, value_binding)
            })
            .unzip();

        let (column_statements, col_params): (Vec<_>, Vec<_>) = col_bindings
            .into_iter()
            .map(|binding| binding.as_tuple())
            .unzip();

        let (value_statements, value_params): (Vec<_>, Vec<_>) = value_bindings
            .into_iter()
            .map(|binding| binding.as_tuple())
            .unzip();

        let stmt = format!(
            "INSERT INTO {} ({}) VALUES ({})",
            table_binding.stmt,
            column_statements.join(", "),
            value_statements.join(", ")
        );

        let mut params = table_binding.params;
        params.extend(col_params.into_iter().flatten());
        params.extend(value_params.into_iter().flatten());

        if self.returning.is_empty() {
            ParameterBinding { stmt, params }
        } else {
            let (ret_stmts, ret_params): (Vec<_>, Vec<_>) = self
                .returning
                .iter()
                .map(|ret| ret.binding(expression_context).as_tuple())
                .unzip();

            let stmt = format!("{} RETURNING {}", stmt, ret_stmts.join(", "));
            params.extend(ret_params.into_iter().flatten());

            ParameterBinding { stmt, params }
        }
    }
}
