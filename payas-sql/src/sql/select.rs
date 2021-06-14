use super::{
    column::Column, order::OrderBy, physical_table::PhysicalTable, predicate::Predicate,
    Expression, ExpressionContext, ParameterBinding,
};

#[derive(Debug, Clone, PartialEq)]
pub struct Select<'a> {
    pub underlying: &'a PhysicalTable,
    pub columns: Vec<&'a Column<'a>>,
    pub predicate: Option<&'a Predicate<'a>>,
    pub order_by: Option<OrderBy<'a>>,
    pub top_level_selection: bool,
}

impl<'a> Expression for Select<'a> {
    fn binding(&self, expression_context: &mut ExpressionContext) -> ParameterBinding {
        let table_binding = self.underlying.binding(expression_context);

        let (col_stmtss, col_paramss): (Vec<_>, Vec<_>) = self
            .columns
            .iter()
            .map(|c| {
                let col_binding = c.binding(expression_context);
                let text_cast = match c {
                    Column::JsonObject(_) | Column::JsonAgg(_) if self.top_level_selection => {
                        "::text"
                    }
                    _ => "",
                };
                (
                    format!("{}{}", col_binding.stmt, text_cast),
                    col_binding.params,
                )
            })
            .unzip();

        let cols_stmts: String = col_stmtss.join(", ");

        let mut params: Vec<_> = col_paramss.into_iter().flatten().collect();
        params.extend(table_binding.params);

        let stmt = match self.predicate {
            // Avoid correct, but inelegant "where true" clause
            Some(Predicate::True) | None => match &self.order_by {
                None => format!("select {} from {}", cols_stmts, table_binding.stmt),
                Some(order_by) => {
                    let order_by_binding = order_by.binding(expression_context);
                    params.extend(order_by_binding.params);

                    format!(
                        "select {} from (select * from {} order by {}) as {}",
                        cols_stmts, table_binding.stmt, order_by_binding.stmt, table_binding.stmt
                    )
                }
            },
            Some(ref predicate) => {
                let predicate_binding = predicate.binding(expression_context);
                params.extend(predicate_binding.params);

                match &self.order_by {
                    None => format!(
                        "select {} from {} where {}",
                        cols_stmts, table_binding.stmt, predicate_binding.stmt
                    ),
                    Some(order_by) => {
                        let order_by_binding = order_by.binding(expression_context);
                        params.extend(order_by_binding.params);

                        format!(
                            "select {} from (select * from {} where {} order by {}) as {}",
                            cols_stmts,
                            table_binding.stmt,
                            predicate_binding.stmt,
                            order_by_binding.stmt,
                            table_binding.stmt
                        )
                    }
                }
            }
        };

        ParameterBinding::new(stmt, params)
    }
}

#[cfg(test)]
mod tests {
    use crate::sql::column::{IntBits, PhysicalColumn, PhysicalColumnType};

    use super::*;

    #[test]
    fn predicated_table() {
        let table = PhysicalTable {
            name: "people".to_string(),
            columns: vec![PhysicalColumn {
                table_name: "people".to_string(),
                column_name: "age".to_string(),
                typ: PhysicalColumnType::Int { bits: IntBits::_16 },
                is_pk: false,
                is_autoincrement: false,
                references: None,
            }],
        };

        let age_col = table.get_column("age").unwrap();
        let age_value_col = Column::Literal(Box::new(5));

        let predicate = Predicate::Eq(&age_col, &age_value_col);

        let selected_cols = vec![&age_col];

        let predicated_table = table.select(selected_cols, Some(&predicate), None, false);

        let mut expression_context = ExpressionContext::default();
        assert_binding!(
            &predicated_table.binding(&mut expression_context),
            r#"select "people"."age" from "people" where "people"."age" = $1"#,
            5
        );
    }

    #[test]
    fn json_object() {
        let table = PhysicalTable {
            name: "people".to_string(),
            columns: vec![
                PhysicalColumn {
                    table_name: "people".to_string(),
                    column_name: "name".to_string(),
                    typ: PhysicalColumnType::String,
                    is_pk: false,
                    is_autoincrement: false,
                    references: None,
                },
                PhysicalColumn {
                    table_name: "people".to_string(),
                    column_name: "age".to_string(),
                    typ: PhysicalColumnType::Int { bits: IntBits::_16 },
                    is_pk: false,
                    is_autoincrement: false,
                    references: None,
                },
            ],
        };

        let age_col = table.get_column("age").unwrap();
        let name_col = table.get_column("name").unwrap();
        let x = Column::JsonObject(vec![
            ("namex".to_string(), &name_col),
            ("agex".to_string(), &age_col),
        ]);
        let selected_table = table.select(vec![&age_col, &x], None, None, true);

        let mut expression_context = ExpressionContext::default();
        assert_binding!(
            &selected_table.binding(&mut expression_context),
            r#"select "people"."age", json_build_object('namex', "people"."name", 'agex', "people"."age")::text from "people""#
        );
    }
}
