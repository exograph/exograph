use super::{Expression, ParameterBinding, SQLParam};
use crate::sql::ExpressionContext;

#[derive(Debug)]
pub enum Column<'a> {
    Physical {
        table_name: String,
        column_name: String,
    },
    Literal(Box<dyn SQLParam>),
    JsonObject(Vec<(String, Column<'a>)>),
    JsonAgg(&'a Column<'a>),
}

impl<'a> Expression for Column<'a> {
    fn binding(&self, expression_context: &mut ExpressionContext) -> ParameterBinding {
        match self {
            Column::Physical {
                table_name,
                column_name,
            } => ParameterBinding::new(format!("{}.{}", table_name, column_name), vec![]),
            Column::Literal(value) => {
                let param_index = expression_context.next_param();
                ParameterBinding::new(format! {"${}", param_index}, vec![value])
            }
            Column::JsonObject(elems) => {
                let (elem_stmt, elem_params): (Vec<_>, Vec<_>) = elems
                    .iter()
                    .map(|elem| {
                        let elem_binding = elem.1.binding(expression_context);
                        (
                            format!("'{}', {}", elem.0, elem_binding.stmt),
                            elem_binding.params,
                        )
                    })
                    .unzip();

                let stmt = format!("json_build_object({})", elem_stmt.join(", "));
                let params = elem_params.into_iter().flatten().collect();
                ParameterBinding::new(stmt, params)
            }
            Column::JsonAgg(column) => {
                let column_binding = column.binding(expression_context);
                let stmt = format!("json_agg({})", column_binding.stmt);
                ParameterBinding::new(stmt, column_binding.params)
            }
        }
    }
}
