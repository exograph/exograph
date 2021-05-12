use super::{column::Column, Expression, ParameterBinding};
#[derive(Debug, Clone)]
pub enum Ordering {
    Asc,
    Desc,
}

#[derive(Debug, Clone)]
pub struct OrderBy<'a>(pub Vec<(&'a Column<'a>, Ordering)>);

impl<'a> Expression for OrderBy<'a> {
    fn binding(&self, expression_context: &mut super::ExpressionContext) -> ParameterBinding {
        let (stmts, params): (Vec<_>, Vec<_>) = self
            .0
            .iter()
            .map(|elem| {
                let column_binding = elem.0.binding(expression_context);
                let order_stmt = match &elem.1 {
                    Ordering::Asc => "ASC",
                    Ordering::Desc => "DESC",
                };
                (
                    format!("{} {}", column_binding.stmt, order_stmt),
                    column_binding.params,
                )
            })
            .unzip();

        ParameterBinding::new(stmts.join(", "), params.into_iter().flatten().collect())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::sql::column::{PhysicalColumn, PhysicalColumnType};
    use crate::sql::ExpressionContext;

    #[test]
    fn single() {
        let age_col = PhysicalColumn {
            table_name: "people".to_string(),
            column_name: "age".to_string(),
            typ: PhysicalColumnType::Int,
        };
        let age_col = Column::Physical(&age_col);

        let order_by = OrderBy(vec![(&age_col, Ordering::Desc)]);

        let mut expression_context = ExpressionContext::new();
        let binding = order_by.binding(&mut expression_context);

        assert_binding!(binding, r#""people"."age" DESC"#);
    }

    #[test]
    fn multiple() {
        let name_col = PhysicalColumn {
            table_name: "people".to_string(),
            column_name: "name".to_string(),
            typ: PhysicalColumnType::String,
        };
        let name_col = Column::Physical(&name_col);

        let age_col = PhysicalColumn {
            table_name: "people".to_string(),
            column_name: "age".to_string(),
            typ: PhysicalColumnType::Int,
        };
        let age_col = Column::Physical(&age_col);

        {
            let order_by = OrderBy(vec![(&name_col, Ordering::Asc), (&age_col, Ordering::Desc)]);

            let mut expression_context = ExpressionContext::new();
            let binding = order_by.binding(&mut expression_context);

            assert_binding!(binding, r#""people"."name" ASC, "people"."age" DESC"#);
        }

        // Reverse the order and it should be refleted in the statement
        {
            let order_by = OrderBy(vec![(&age_col, Ordering::Desc), (&name_col, Ordering::Asc)]);

            let mut expression_context = ExpressionContext::new();
            let binding = order_by.binding(&mut expression_context);

            assert_binding!(binding, r#""people"."age" DESC, "people"."name" ASC"#);
        }
    }
}
