use crate::{Limit, Offset};

use super::{
    column::Column, group_by::GroupBy, order::OrderBy, predicate::ConcretePredicate, table::Table,
    ExpressionBuilder, SQLBuilder,
};

/// A select statement
#[derive(Debug, PartialEq)]
pub struct Select<'a> {
    /// The table to select from
    pub table: Table<'a>,
    /// The columns to select
    pub columns: Vec<Column<'a>>,
    /// The predicate to filter the rows
    pub predicate: ConcretePredicate<'a>,
    /// The order by clause
    pub order_by: Option<OrderBy<'a>>,
    /// The offset clause
    pub offset: Option<Offset>,
    /// The limit clause
    pub limit: Option<Limit>,
    /// The group by clause
    pub group_by: Option<GroupBy<'a>>,
    /// Whether this is a top-level selection. This is used to put the `::text` cast on a top-level select statement
    /// This way, we can grab the JSON as a string and return it to the user as is. Specifically, we don't want to
    /// decode into a JSON object and then re-encode it as a string.
    pub top_level_selection: bool,
}

impl<'a> ExpressionBuilder for Select<'a> {
    fn build(&self, builder: &mut SQLBuilder) {
        builder.push_str("SELECT ");

        // Columns
        builder.push_iter(self.columns.iter(), ", ", |builder, col| {
            col.build(builder);

            if self.top_level_selection && matches!(col, Column::JsonObject(_) | Column::JsonAgg(_))
            {
                // See the comment on `top_level_selection` for why we do this
                builder.push_str("::text");
            }
        });

        builder.push_str(" FROM ");
        self.table.build(builder);

        // Avoid correct, but inelegant "WHERE TRUE" clause
        if self.predicate != ConcretePredicate::True {
            builder.push_str(" WHERE ");
            self.predicate.build(builder);
        }
        if let Some(group_by) = &self.group_by {
            builder.push_space();
            group_by.build(builder);
        }
        if let Some(order_by) = &self.order_by {
            builder.push_space();
            order_by.build(builder);
        }
        if let Some(limit) = &self.limit {
            builder.push_space();
            limit.build(builder);
        }
        if let Some(offset) = &self.offset {
            builder.push_space();
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
                    name: "name".to_string(),
                    typ: PhysicalColumnType::String { length: None },
                    ..Default::default()
                },
                PhysicalColumn {
                    table_name: "people".to_string(),
                    name: "age".to_string(),
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
        let table = Table::Physical(&physical_table);
        let selected_table = Select {
            table,
            columns: vec![age_col2, json_col],
            predicate: ConcretePredicate::True,
            order_by: None,
            limit: None,
            offset: None,
            group_by: None,
            top_level_selection: true,
        };

        assert_binding!(
            selected_table.to_sql(),
            r#"SELECT "people"."age", json_build_object('namex', "people"."name", 'agex', "people"."age")::text FROM "people""#
        );
    }
}
