use crate::{Limit, Offset};

use super::{
    column::Column, group_by::GroupBy, order::OrderBy, predicate::ConcretePredicate,
    table::TableQuery, Expression, ParameterBinding,
};

#[derive(Debug, PartialEq)]
pub struct Select<'a> {
    pub underlying: TableQuery<'a>,
    pub columns: Vec<Column<'a>>,
    pub predicate: ConcretePredicate<'a>,
    pub order_by: Option<OrderBy<'a>>,
    pub offset: Option<Offset>,
    pub limit: Option<Limit>,
    pub group_by: Option<GroupBy<'a>>,
    pub top_level_selection: bool,
}

impl<'a> Expression for Select<'a> {
    fn binding(&self) -> ParameterBinding {
        let table_binding = self.underlying.binding();

        let col_stmts: Vec<_> = self
            .columns
            .iter()
            .map(|c| {
                let col_binding = c.binding();
                match c {
                    Column::JsonObject(_) | Column::JsonAgg(_) if self.top_level_selection => {
                        ParameterBinding::Cast(Box::new(col_binding), "text")
                    }
                    _ => col_binding,
                }
            })
            .collect();

        let predicate_part = match &self.predicate {
            // Avoid correct, but inelegant "where true" clause
            ConcretePredicate::True => None,
            predicate => Some(Box::new(ParameterBinding::Predicate(Box::new(
                predicate.binding(),
            )))),
        };

        let group_by_part = self
            .group_by
            .as_ref()
            .map(|group_by| Box::new(group_by.binding()));

        let order_by_part = self
            .order_by
            .as_ref()
            .map(|order_by| Box::new(order_by.binding()));

        let limit_part = self.limit.as_ref().map(|limit| Box::new(limit.binding()));

        let offset_part = self
            .offset
            .as_ref()
            .map(|offset| Box::new(offset.binding()));

        if order_by_part.is_some() || limit_part.is_some() || offset_part.is_some() {
            let inner_select = ParameterBinding::Select {
                columns: vec![ParameterBinding::Star(Some(
                    &self.underlying.base_table().name,
                ))],
                from: Box::new(table_binding),
                predicate: predicate_part,
                group_by: group_by_part,
                order_by: order_by_part,
                limit: limit_part,
                offset: offset_part,
                alias: None,
                nested: true,
            };

            ParameterBinding::Select {
                columns: col_stmts,
                from: Box::new(inner_select),
                predicate: None,
                group_by: None,
                order_by: None,
                limit: None,
                offset: None,
                alias: Some(self.underlying.base_table().name.clone()),
                nested: false,
            }
            // let base_table_stmt = self.underlying.base_table().binding().stmt;
            // // If we just "select *", we may get duplicated columns from a join statement, so we only pick the columns of the base table (left-most table in the join)
            // format!(
            //     "select {cols_stmts} from (select {base_table_stmt}.* from {table_binding_stmt}{conditions}) as {table_binding_stmt}",
            // )
            // todo!("Select::binding")
        } else {
            ParameterBinding::Select {
                columns: col_stmts,
                from: Box::new(table_binding),
                predicate: predicate_part,
                group_by: group_by_part,
                order_by: None,
                limit: None,
                offset: None,
                alias: None,
                nested: false,
            }
        }
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

        let predicate = ConcretePredicate::Eq(age_col, age_value_col);

        let age_col = physical_table.get_column("age").unwrap();
        let selected_cols = vec![age_col];

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

        let binding = predicated_table.binding();

        assert_binding!(
            binding,
            r#"SELECT "people"."age" FROM (SELECT "people".* FROM "people" WHERE "people"."age" = $1 LIMIT $2 OFFSET $3) AS "people""#,
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
            vec![age_col2, json_col],
            ConcretePredicate::True,
            None,
            None,
            None,
            None,
            true,
        );

        assert_binding!(
            selected_table.binding(),
            r#"SELECT "people"."age", json_build_object('namex', "people"."name", 'agex', "people"."age")::text FROM "people""#
        );
    }
}
