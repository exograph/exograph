use crate::{Limit, Offset};

use super::{
    column::Column, group_by::GroupBy, order::OrderBy, predicate::ConcretePredicate,
    table::TableQuery, ExpressionBuilder, SQLBuilder,
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

impl<'a> ExpressionBuilder for Select<'a> {
    fn build(&self, builder: &mut SQLBuilder) {
        builder.push_str("SELECT ");

        builder.push_iter(self.columns.iter(), ", ", |builder, col| {
            col.build(builder);
            match col {
                Column::JsonObject(_) | Column::JsonAgg(_) if self.top_level_selection => {
                    builder.push_str("::text");
                }
                _ => {}
            }
        });

        builder.push_str(" FROM ");

        self.underlying.build(builder);

        // Avoid correct, but inelegant "where true" clause
        if self.predicate != ConcretePredicate::True {
            builder.push_str(" WHERE ");
            self.predicate.build(builder);
        }
        if let Some(group_by) = &self.group_by {
            builder.push(' ');
            group_by.build(builder);
        }
        if let Some(order_by) = &self.order_by {
            builder.push(' ');
            order_by.build(builder);
        }
        if let Some(limit) = &self.limit {
            limit.build(builder);
        }
        if let Some(offset) = &self.offset {
            offset.build(builder);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        sql::json_object::{JsonObject, JsonObjectElement},
        sql::physical_column::{IntBits, PhysicalColumn, PhysicalColumnType},
        PhysicalTable,
    };

    use super::*;

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
        let json_col = Column::JsonObject(JsonObject(vec![
            JsonObjectElement::new("namex".to_string(), name_col),
            JsonObjectElement::new("agex".to_string(), age_col),
        ]));
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
            selected_table.into_sql(),
            r#"SELECT "people"."age", json_build_object('namex', "people"."name", 'agex', "people"."age")::text FROM "people""#
        );
    }
}
