use maybe_owned::MaybeOwned;

use super::{
    column::Column, limit::Limit, offset::Offset, order::OrderBy, physical_table::PhysicalTable,
    predicate::Predicate, Expression, ExpressionContext, ParameterBinding,
};

#[derive(Debug, PartialEq)]
pub struct Select<'a> {
    pub underlying: &'a PhysicalTable,
    pub columns: Vec<MaybeOwned<'a, Column<'a>>>,
    pub predicate: Option<&'a Predicate<'a>>,
    pub order_by: Option<OrderBy<'a>>,
    pub offset: Option<Offset>,
    pub limit: Option<Limit>,
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
                let text_cast = match c.as_ref() {
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
            Some(predicate) => {
                let predicate_binding = predicate.binding(expression_context);
                params.extend(predicate_binding.params);

                let order_by_part = self.order_by.as_ref().map(|order_by| {
                    let order_by_binding = order_by.binding(expression_context);
                    params.extend(order_by_binding.params);

                    format!(" order by {}", order_by_binding.stmt)
                });

                let limit_part = self.limit.as_ref().map(|limit| {
                    let limit_binding = limit.binding(expression_context);
                    params.extend(limit_binding.params);
                    format!(" {}", limit_binding.stmt)
                });

                let offset_part = self.offset.as_ref().map(|offset| {
                    let offset_binding = offset.binding(expression_context);
                    params.extend(offset_binding.params);
                    format!(" {}", offset_binding.stmt)
                });

                if order_by_part.is_some() || limit_part.is_some() || offset_part.is_some() {
                    let conditions = format!(
                        "{}{}{}",
                        order_by_part.unwrap_or_default(),
                        limit_part.unwrap_or_default(),
                        offset_part.unwrap_or_default()
                    );

                    format!(
                        "select {} from (select * from {} where {}{}) as {}",
                        cols_stmts,
                        table_binding.stmt,
                        predicate_binding.stmt,
                        conditions,
                        table_binding.stmt
                    )
                } else {
                    format!(
                        "select {} from {} where {}",
                        cols_stmts, table_binding.stmt, predicate_binding.stmt
                    )
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
            }],
        };

        let age_col = table.get_column("age").unwrap();
        let age_value_col = Column::Literal(Box::new(5));

        let predicate = Predicate::Eq(age_col.into(), age_value_col.into());

        let age_col = table.get_column("age").unwrap();
        let selected_cols = vec![age_col.into()];

        let predicated_table = table.select(
            selected_cols,
            Some(&predicate),
            None,
            Some(Offset(10)),
            Some(Limit(20)),
            false,
        );

        let mut expression_context = ExpressionContext::default();
        let binding = predicated_table.binding(&mut expression_context);
        println!("{:?}", binding.params);
        assert_binding!(
            &binding,
            r#"select "people"."age" from (select * from "people" where "people"."age" = $1 LIMIT $2 OFFSET $3) as "people""#,
            5,
            20i64,
            10i64
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
                    typ: PhysicalColumnType::String { length: None },
                    is_pk: false,
                    is_autoincrement: false,
                },
                PhysicalColumn {
                    table_name: "people".to_string(),
                    column_name: "age".to_string(),
                    typ: PhysicalColumnType::Int { bits: IntBits::_16 },
                    is_pk: false,
                    is_autoincrement: false,
                },
            ],
        };

        let age_col = table.get_column("age").unwrap();
        let name_col = table.get_column("name").unwrap();
        let json_col = Column::JsonObject(vec![
            ("namex".to_string(), name_col.into()),
            ("agex".to_string(), age_col.into()),
        ]);
        let selected_table = table.select(
            vec![table.get_column("age").unwrap().into(), json_col.into()],
            None,
            None,
            None,
            None,
            true,
        );

        let mut expression_context = ExpressionContext::default();
        assert_binding!(
            &selected_table.binding(&mut expression_context),
            r#"select "people"."age", json_build_object('namex', "people"."name", 'agex', "people"."age")::text from "people""#
        );
    }
}
