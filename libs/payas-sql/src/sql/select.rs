use maybe_owned::MaybeOwned;

use crate::{Limit, Offset};

use super::{
    column::Column, group_by::GroupBy, order::OrderBy, predicate::Predicate, table::TableQuery,
    Expression, ExpressionContext, ParameterBinding,
};

#[derive(Debug, PartialEq)]
pub struct Select<'a> {
    pub underlying: TableQuery<'a>,
    pub columns: Vec<MaybeOwned<'a, Column<'a>>>,
    pub predicate: MaybeOwned<'a, Predicate<'a>>,
    pub order_by: Option<OrderBy<'a>>,
    pub offset: Option<Offset>,
    pub limit: Option<Limit>,
    pub group_by: Option<GroupBy<'a>>,
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

        let group_by_part = self.group_by.as_ref().map(|group_by| {
            let binding = group_by.binding(expression_context);
            params.extend(binding.params);
            format!(" {}", binding.stmt)
        });

        let table_binding_stmt = table_binding.stmt;
        let stmt = if order_by_part.is_some() || limit_part.is_some() || offset_part.is_some() {
            let conditions = format!(
                "{}{}{}{}{}",
                predicate_part,
                group_by_part.unwrap_or_default(),
                order_by_part.unwrap_or_default(),
                limit_part.unwrap_or_default(),
                offset_part.unwrap_or_default(),
            );

            let base_table_stmt = self
                .underlying
                .base_table()
                .binding(expression_context)
                .stmt;
            // If we just "select *", we may get duplicated columns from a join statement, so we only pick the columns of the base table (left-most table in the join)
            format!(
                "select {cols_stmts} from (select {base_table_stmt}.* from {table_binding_stmt}{conditions}) as {table_binding_stmt}",
            )
        } else {
            format!(
                "select {cols_stmts} from {table_binding_stmt}{predicate_part}{}",
                group_by_part.unwrap_or_default()
            )
        };

        ParameterBinding::new(stmt, params)
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        sql::{
            column::{IntBits, PhysicalColumn, PhysicalColumnType},
            SQLParamContainer,
        },
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
        let age_value_col = Column::Literal(SQLParamContainer::new(5));

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
            None,
            false,
        );

        let mut expression_context = ExpressionContext::default();
        let binding = predicated_table.binding(&mut expression_context);

        assert_binding!(
            binding,
            r#"select "people"."age" from (select "people".* from "people" WHERE "people"."age" = $1 LIMIT $2 OFFSET $3) as "people""#,
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
            ("namex".to_string(), name_col),
            ("agex".to_string(), age_col),
        ]);
        let table = TableQuery::Physical(&physical_table);
        let selected_table = table.select(
            vec![age_col2.into(), json_col.into()],
            Predicate::True,
            None,
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
