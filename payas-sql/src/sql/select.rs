use maybe_owned::MaybeOwned;

use crate::{Limit, Offset};

use super::{
    column::Column, order::OrderBy, predicate::Predicate, table::TableQuery, Expression,
    ExpressionContext, ParameterBinding,
};

#[derive(Debug, PartialEq)]
pub struct Select<'a> {
    pub underlying: TableQuery<'a>,
    pub columns: Vec<MaybeOwned<'a, Column<'a>>>,
    pub predicate: MaybeOwned<'a, Predicate<'a>>,
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

        let predicate_part = match self.predicate.as_ref() {
            // Avoid correct, but inelegant "where true" clause
            Predicate::True => "".to_string(),
            predicate => {
                let binding = predicate.binding(expression_context);
                params.extend(binding.params);
                format!(" WHERE {}", binding.stmt)
            }
        };

        let order_by_part = self.order_by.as_ref().map(|order_by| {
            let binding = order_by.binding(expression_context);
            params.extend(binding.params);

            format!(" {}", binding.stmt)
        });

        let limit_part = self.limit.as_ref().map(|limit| {
            let binding = limit.binding(expression_context);
            params.extend(binding.params);
            format!(" {}", binding.stmt)
        });

        let offset_part = self.offset.as_ref().map(|offset| {
            let binding = offset.binding(expression_context);
            params.extend(binding.params);
            format!(" {}", binding.stmt)
        });

        let stmt = if order_by_part.is_some() || limit_part.is_some() || offset_part.is_some() {
            let conditions = format!(
                "{}{}{}{}",
                predicate_part,
                order_by_part.unwrap_or_default(),
                limit_part.unwrap_or_default(),
                offset_part.unwrap_or_default()
            );

            format!(
                "select {} from (select * from {}{}) as {}",
                cols_stmts, table_binding.stmt, conditions, table_binding.stmt
            )
        } else {
            format!(
                "select {} from {}{}",
                cols_stmts, table_binding.stmt, predicate_part
            )
        };

        ParameterBinding::new(stmt, params)
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        sql::column::{IntBits, PhysicalColumn, PhysicalColumnType},
        PhysicalTable,
    };

    use super::*;

    #[test]
    fn predicated_table() {
        let physical_table = PhysicalTable {
            name: "people".to_string(),
            columns: vec![PhysicalColumn {
                table_name: "people".to_string(),
                column_name: "age".to_string(),
                typ: PhysicalColumnType::Int { bits: IntBits::_16 },
                ..Default::default()
            }],
        };

        let age_col = physical_table.get_column("age").unwrap();
        let age_value_col = Column::Literal(MaybeOwned::Owned(Box::new(5)));

        let predicate = Predicate::Eq(age_col.into(), age_value_col.into());

        let age_col = physical_table.get_column("age").unwrap();
        let selected_cols = vec![age_col.into()];

        let table = TableQuery::Physical(&physical_table);

        let predicated_table = table.select(
            selected_cols,
            predicate,
            None,
            Some(Offset(10)),
            Some(Limit(20)),
            false,
        );

        let mut expression_context = ExpressionContext::default();
        let binding = predicated_table.binding(&mut expression_context);

        assert_binding!(
            binding,
            r#"select "people"."age" from (select * from "people" WHERE "people"."age" = $1 LIMIT $2 OFFSET $3) as "people""#,
            5,
            20i64,
            10i64
        );
    }

    #[test]
    fn json_object() {
        let physical_table = PhysicalTable {
            name: "people".to_string(),
            columns: vec![
                PhysicalColumn {
                    table_name: "people".to_string(),
                    column_name: "name".to_string(),
                    typ: PhysicalColumnType::String { length: None },
                    ..Default::default()
                },
                PhysicalColumn {
                    table_name: "people".to_string(),
                    column_name: "age".to_string(),
                    typ: PhysicalColumnType::Int { bits: IntBits::_16 },
                    ..Default::default()
                },
            ],
        };

        let age_col = physical_table.get_column("age").unwrap();
        let age_col2 = physical_table.get_column("age").unwrap();

        let name_col = physical_table.get_column("name").unwrap();
        let json_col = Column::JsonObject(vec![
            ("namex".to_string(), name_col.into()),
            ("agex".to_string(), age_col.into()),
        ]);
        let table = TableQuery::Physical(&physical_table);
        let selected_table = table.select(
            vec![age_col2.into(), json_col.into()],
            Predicate::True,
            None,
            None,
            None,
            true,
        );

        let mut expression_context = ExpressionContext::default();
        assert_binding!(
            selected_table.binding(&mut expression_context),
            r#"select "people"."age", json_build_object('namex', "people"."name", 'agex', "people"."age")::text from "people""#
        );
    }
}
