use super::{column::PhysicalColumn, Expression, ParameterBinding};
#[derive(Debug, Clone, PartialEq)]
pub enum Ordering {
    Asc,
    Desc,
}

#[derive(Debug, PartialEq)]
pub struct OrderBy<'a>(pub Vec<(&'a PhysicalColumn, Ordering)>);

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

        ParameterBinding::new(
            format!("ORDER BY {}", stmts.join(", ")),
            params.into_iter().flatten().collect(),
        )
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::sql::column::{IntBits, PhysicalColumn, PhysicalColumnType};
    use crate::sql::ExpressionContext;

    #[test]
    fn single() {
        let age_col = PhysicalColumn {
            table_name: "people".to_string(),
            column_name: "age".to_string(),
            typ: PhysicalColumnType::Int { bits: IntBits::_16 },
            is_pk: false,
            is_autoincrement: false,
        };

        let order_by = OrderBy(vec![(&age_col, Ordering::Desc)]);

        let mut expression_context = ExpressionContext::default();
        let binding = order_by.binding(&mut expression_context);

        assert_binding!(binding, r#"ORDER BY "people"."age" DESC"#);
    }

    #[test]
    fn multiple() {
        let name_col = PhysicalColumn {
            table_name: "people".to_string(),
            column_name: "name".to_string(),
            typ: PhysicalColumnType::String { length: None },
            is_pk: false,
            is_autoincrement: false,
        };

        let age_col = PhysicalColumn {
            table_name: "people".to_string(),
            column_name: "age".to_string(),
            typ: PhysicalColumnType::Int { bits: IntBits::_16 },
            is_pk: false,
            is_autoincrement: false,
        };

        {
            let order_by = OrderBy(vec![(&name_col, Ordering::Asc), (&age_col, Ordering::Desc)]);

            let mut expression_context = ExpressionContext::default();
            let binding = order_by.binding(&mut expression_context);

            assert_binding!(
                binding,
                r#"ORDER BY "people"."name" ASC, "people"."age" DESC"#
            );
        }

        // Reverse the order and it should be refleted in the statement
        {
            let order_by = OrderBy(vec![(&age_col, Ordering::Desc), (&name_col, Ordering::Asc)]);

            let mut expression_context = ExpressionContext::default();
            let binding = order_by.binding(&mut expression_context);

            assert_binding!(
                binding,
                r#"ORDER BY "people"."age" DESC, "people"."name" ASC"#
            );
        }
    }
}
