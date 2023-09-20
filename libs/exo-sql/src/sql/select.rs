// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::collections::HashMap;

use crate::{Database, Limit, Offset};

use super::{
    column::Column, group_by::GroupBy, order::OrderBy, predicate::ConcretePredicate, table::Table,
    ExpressionBuilder, SQLBuilder,
};

/// A select statement
#[derive(Debug, PartialEq)]
pub struct Select {
    /// The table to select from
    pub table: Table,
    /// The columns to select
    pub columns: Vec<Column>,
    /// The predicate to filter the rows
    pub predicate: ConcretePredicate,
    /// The order by clause
    pub order_by: Option<OrderBy>,
    /// The offset clause
    pub offset: Option<Offset>,
    /// The limit clause
    pub limit: Option<Limit>,
    /// The group by clause
    pub group_by: Option<GroupBy>,
    /// Whether this is a top-level selection. This is used to put the `::text` cast on a top-level select statement
    /// This way, we can grab the JSON as a string and return it to the user as is. Specifically, we don't want to
    /// decode into a JSON object and then re-encode it as a string.
    pub top_level_selection: bool,
}

impl ExpressionBuilder for Select {
    fn build(&self, database: &Database, builder: &mut SQLBuilder) {
        let table_alias_map = match &self.table {
            // If the underlying table is a subselect, we need to add it to the table alias map so
            // tha the other parts (select, where, etc.) can use the alias instead of the subselect's table name
            Table::SubSelect {
                alias: Some((alias_name, table_name)),
                ..
            } => HashMap::from([(table_name.clone(), alias_name.clone())]),
            _ => HashMap::new(),
        };

        builder.push_str("SELECT ");

        // Columns
        builder.with_table_alias_map(table_alias_map.clone(), |builder| {
            builder.push_iter(self.columns.iter(), ", ", |builder, col| {
                col.build(database, builder);

                if self.top_level_selection
                    && matches!(col, Column::JsonObject(_) | Column::JsonAgg(_))
                {
                    // See the comment on `top_level_selection` for why we do this
                    builder.push_str("::text");
                }
            });
        });

        builder.push_str(" FROM ");
        self.table.build(database, builder);

        builder.with_table_alias_map(table_alias_map, |builder| {
            // Avoid correct, but inelegant "WHERE TRUE" clause
            if self.predicate != ConcretePredicate::True {
                builder.push_str(" WHERE ");
                self.predicate.build(database, builder);
            }
            if let Some(group_by) = &self.group_by {
                builder.push_space();
                group_by.build(database, builder);
            }
            if let Some(order_by) = &self.order_by {
                builder.push_space();
                order_by.build(database, builder);
            }
            if let Some(limit) = &self.limit {
                builder.push_space();
                limit.build(database, builder);
            }
            if let Some(offset) = &self.offset {
                builder.push_space();
                offset.build(database, builder);
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        schema::{
            database_spec::DatabaseSpec,
            table_spec::TableSpec,
            test_helper::{int_column, pk_column, string_column},
        },
        sql::json_object::{JsonObject, JsonObjectElement},
        PhysicalTableName,
    };

    use super::*;

    #[test]
    fn json_object() {
        let database = DatabaseSpec::new(vec![TableSpec::new(
            PhysicalTableName {
                name: "people".to_owned(),
                schema: None,
            },
            vec![pk_column("id"), string_column("name"), int_column("age")],
        )])
        .to_database();

        let table_id = database
            .get_table_id(&PhysicalTableName::new("people", None))
            .unwrap();
        let age_col_id = database.get_column_id(table_id, "age").unwrap();
        let age_col2_id = database.get_column_id(table_id, "age").unwrap();
        let name_col_id = database.get_column_id(table_id, "name").unwrap();

        let json_col = Column::JsonObject(JsonObject(vec![
            JsonObjectElement::new("namex".to_string(), Column::physical(name_col_id, None)),
            JsonObjectElement::new("agex".to_string(), Column::physical(age_col_id, None)),
        ]));
        let table = Table::physical(table_id, None);
        let selected_table = Select {
            table,
            columns: vec![Column::physical(age_col2_id, None), json_col],
            predicate: ConcretePredicate::True,
            order_by: None,
            limit: None,
            offset: None,
            group_by: None,
            top_level_selection: true,
        };

        assert_binding!(
            selected_table.to_sql(&database),
            r#"SELECT "people"."age", json_build_object('namex', "people"."name", 'agex', "people"."age")::text FROM "people""#
        );
    }
}
