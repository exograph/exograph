use super::{
    column::{Column, PhysicalColumn},
    Expression, ExpressionContext, ParameterBinding, PhysicalTable,
};

#[derive(Debug)]
pub struct Insert<'a> {
    pub table: &'a PhysicalTable,
    pub column_names: Vec<&'a PhysicalColumn>,
    pub column_values_seq: Vec<Vec<&'a Column<'a>>>,
    pub returning: Vec<&'a Column<'a>>,
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
