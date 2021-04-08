use super::{
    column::Column, order::OrderBy, predicate::Predicate, Expression, ExpressionContext,
    ParameterBinding,
};
use itertools::Itertools;

#[derive(Debug)]
pub struct PhysicalTable<'a> {
    pub name: String,
    pub columns: Vec<Column<'a>>, // Really Column::Physical, but we can't express that
}

impl<'a> PhysicalTable<'a> {
    // TODO: Consider column names that are different than field names
    pub fn get_column(&self, name: &str) -> Option<&Column> {
        self.columns.iter().find(|column| match column {
            Column::Physical {
                table_name: _,
                column_name,
            } => column_name.as_str() == name,
            _ => false,
        })
    }

    pub fn select<'b>(
        &'b self,
        columns: Vec<&'b Column>,
        predicate: Option<&'b Predicate<'b>>,
        order_by: Option<OrderBy<'a>>,
    ) -> SelectionTable {
        SelectionTable {
            underlying: self,
            columns: columns,
            predicate,
            order_by,
        }
    }
}

impl Expression for PhysicalTable<'_> {
    fn binding(&self, _expression_context: &mut ExpressionContext) -> ParameterBinding {
        ParameterBinding::new(format!(r#""{}""#, self.name.clone()), vec![])
    }
}
pub struct SelectionTable<'a> {
    pub underlying: &'a PhysicalTable<'a>,
    pub columns: Vec<&'a Column<'a>>,
    pub predicate: Option<&'a Predicate<'a>>,
    pub order_by: Option<OrderBy<'a>>,
}

impl<'a> Expression for SelectionTable<'a> {
    fn binding(&self, expression_context: &mut ExpressionContext) -> ParameterBinding {
        let table_binding = self.underlying.binding(expression_context);

        let (col_stmtss, col_paramss): (Vec<_>, Vec<_>) = self
            .columns
            .iter()
            .map(|c| {
                let col_binding = c.binding(expression_context);
                let text_cast = match c {
                    Column::Physical { .. } | Column::Literal(_) | Column::SingleSelect { .. } => {
                        ""
                    }
                    Column::JsonObject(_) | Column::JsonAgg(_) => "::text",
                };
                (
                    format!("{}{}", col_binding.stmt, text_cast),
                    col_binding.params,
                )
            })
            .unzip();

        let cols_stmts: String = col_stmtss
            .into_iter()
            .map(|s| s.to_string())
            .intersperse(String::from(", "))
            .collect();

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
                            "select {} from (select * from {} order by {} where {}) as {}",
                            cols_stmts,
                            table_binding.stmt,
                            order_by_binding.stmt,
                            predicate_binding.stmt,
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
    use super::*;

    #[test]
    fn predicated_table() {
        let table_name = "people";
        let physical_table = PhysicalTable {
            name: table_name.to_string(),
            columns: vec![Column::Physical {
                table_name: "people".to_string(),
                column_name: "age".to_string(),
            }],
        };

        let age_col = Column::Physical {
            table_name: table_name.to_string(),
            column_name: "age".to_string(),
        };
        let age_value_col = Column::Literal(Box::new(5));
        let predicate = Predicate::Eq(&age_col, &age_value_col);

        let age_selected_col = Column::Physical {
            table_name: table_name.to_string(),
            column_name: "age".to_string(),
        };
        let selected_cols = vec![&age_selected_col];

        let predicated_table = physical_table.select(selected_cols, Some(&predicate), None);

        let mut expression_context = ExpressionContext::new();
        assert_binding!(
            &predicated_table.binding(&mut expression_context),
            r#"select "people"."age" from "people" where "people"."age" = $1"#,
            5
        );
    }

    #[test]
    fn json_object() {
        let table_name = "people";
        let _physical_table = PhysicalTable {
            name: table_name.to_string(),
            columns: vec![
                Column::Physical {
                    table_name: "people".to_string(),
                    column_name: "name".to_string(),
                },
                Column::Physical {
                    table_name: "people".to_string(),
                    column_name: "age".to_string(),
                },
            ],
        };

        let _age_col = Column::Physical {
            table_name: table_name.to_string(),
            column_name: "age".to_string(),
        };

        // let selected_table = physical_table.select(
        //     &vec![
        //         &age_col,
        //         &Column::JsonObject(vec![
        //             (
        //                 "namex".to_string(),
        //                 Column::Physical(table_name.to_string(), "name".to_string()),
        //             ),
        //             (
        //                 "agex".to_string(),
        //                 Column::Physical(table_name.to_string(), "age".to_string()),
        //             ),
        //         ]),
        //     ],
        //     None,
        // );

        // let mut expression_context = ExpressionContext::new();
        // assert_binding!(&selected_table.binding(&mut expression_context), "select people.age, json_build_object('namex', people.name, 'agex', people.age) from people");
    }
}
