// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use crate::{ExpressionBuilder, SQLBuilder, column::Column, vector::VectorDistance};
use exo_sql_core::{
    Database,
    predicate::{CaseSensitivity, NumericComparator, Predicate},
};

pub type ConcretePredicate = Predicate<Column>;

impl ExpressionBuilder for ConcretePredicate {
    /// Build a predicate into a SQL string.
    fn build(&self, database: &Database, builder: &mut SQLBuilder) {
        match &self {
            ConcretePredicate::True => builder.push_str("TRUE"),
            ConcretePredicate::False => builder.push_str("FALSE"),
            ConcretePredicate::Eq(column1, column2) => {
                if column2 == &Column::Null {
                    column1.build(database, builder);
                    builder.push_str(" IS NULL");
                } else {
                    relational_combine(column1, column2, "=", database, builder)
                }
            }
            ConcretePredicate::Neq(column1, column2) => {
                if column2 == &Column::Null {
                    column1.build(database, builder);
                    builder.push_str(" IS NOT NULL");
                } else {
                    relational_combine(column1, column2, "<>", database, builder)
                }
            }
            ConcretePredicate::Lt(column1, column2) => {
                relational_combine(column1, column2, "<", database, builder)
            }
            ConcretePredicate::Lte(column1, column2) => {
                relational_combine(column1, column2, "<=", database, builder)
            }
            ConcretePredicate::Gt(column1, column2) => {
                relational_combine(column1, column2, ">", database, builder)
            }
            ConcretePredicate::Gte(column1, column2) => {
                relational_combine(column1, column2, ">=", database, builder)
            }
            ConcretePredicate::In(column1, column2) => {
                relational_combine(column1, column2, "IN", database, builder)
            }

            ConcretePredicate::StringLike(column1, column2, case_sensitivity) => {
                relational_combine(
                    column1,
                    column2,
                    if *case_sensitivity == CaseSensitivity::Insensitive {
                        "ILIKE"
                    } else {
                        "LIKE"
                    },
                    database,
                    builder,
                )
            }
            // we use the postgres concat operator (||) in order to handle both literals and column references
            ConcretePredicate::StringStartsWith(column1, column2) => {
                column1.build(database, builder);
                builder.push_str(" LIKE ");
                column2.build(database, builder);
                builder.push_str(" || '%'");
            }
            ConcretePredicate::StringEndsWith(column1, column2) => {
                column1.build(database, builder);
                builder.push_str(" LIKE '%' || ");
                column2.build(database, builder);
            }
            ConcretePredicate::JsonContains(column1, column2) => {
                relational_combine(column1, column2, "@>", database, builder)
            }
            ConcretePredicate::JsonContainedBy(column1, column2) => {
                relational_combine(column1, column2, "<@", database, builder)
            }
            ConcretePredicate::JsonMatchKey(column1, column2) => {
                relational_combine(column1, column2, "?", database, builder)
            }
            ConcretePredicate::JsonMatchAnyKey(column1, column2) => {
                relational_combine(column1, column2, "?|", database, builder)
            }
            ConcretePredicate::JsonMatchAllKeys(column1, column2) => {
                relational_combine(column1, column2, "?&", database, builder)
            }

            ConcretePredicate::VectorDistance(
                column1,
                column2,
                distance_op,
                numeric_comp_op,
                numeric_value,
            ) => {
                VectorDistance::new(column1, column2, *distance_op).build(database, builder);
                builder.push_space();
                match numeric_comp_op {
                    NumericComparator::Eq => builder.push_str("="),
                    NumericComparator::Neq => builder.push_str("<>"),
                    NumericComparator::Lt => builder.push_str("<"),
                    NumericComparator::Lte => builder.push_str("<="),
                    NumericComparator::Gt => builder.push_str(">"),
                    NumericComparator::Gte => builder.push_str(">="),
                }
                builder.push_space();
                numeric_value.build(database, builder);
            }

            ConcretePredicate::And(predicate1, predicate2) => {
                logical_combine(predicate1, predicate2, "AND", database, builder)
            }
            ConcretePredicate::Or(predicate1, predicate2) => {
                logical_combine(predicate1, predicate2, "OR", database, builder)
            }
            ConcretePredicate::Not(predicate) => {
                builder.push_str("NOT(");
                predicate.build(database, builder);
                builder.push(')');
            }
        }
    }
}

/// Combine two expressions with a relational operator.
fn relational_combine<'a, E1: ExpressionBuilder, E2: ExpressionBuilder>(
    left: &'a E1,
    right: &'a E2,
    op: &'static str,
    database: &Database,
    builder: &mut SQLBuilder,
) {
    left.build(database, builder);
    builder.push_space();
    builder.push_str(op);
    builder.push_space();
    right.build(database, builder);
}

/// Combine two expressions with a logical binary operator.
fn logical_combine<'a, E1: ExpressionBuilder, E2: ExpressionBuilder>(
    left: &'a E1,
    right: &'a E2,
    op: &'static str,
    database: &Database,
    builder: &mut SQLBuilder,
) {
    builder.push('(');
    left.build(database, builder);
    builder.push_space();
    builder.push_str(op);
    builder.push_space();
    right.build(database, builder);
    builder.push(')');
}

#[cfg(test)]
#[macro_use]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::test_database_builder::*;
    use exo_sql_core::{ColumnId, Database, SQLParamContainer, SchemaObjectName};

    use multiplatform_test::multiplatform_test;

    #[multiplatform_test]
    fn true_predicate() {
        let database = Database::default();
        assert_binding!(ConcretePredicate::True.to_sql(&database), "TRUE");
    }

    #[multiplatform_test]
    fn false_predicate() {
        let database = Database::default();
        assert_binding!(ConcretePredicate::False.to_sql(&database), "FALSE");
    }

    #[multiplatform_test]
    fn eq_predicate() {
        let database = DatabaseBuilder::new()
            .table("people", vec![pk("id"), int("age")])
            .build();

        let people_table_id = database
            .get_table_id(&SchemaObjectName::new("people", None))
            .unwrap();
        let age_column_id = database.get_column_id(people_table_id, "age").unwrap();

        let age_col = Column::physical(age_column_id, None);
        let age_value_col = Column::Param(SQLParamContainer::i32(5));

        let predicate = Predicate::Eq(age_col, age_value_col);

        assert_binding!(predicate.to_sql(&database), r#""people"."age" = $1"#, 5);
    }

    #[multiplatform_test]
    fn and_predicate() {
        let database = DatabaseBuilder::new()
            .table("people", vec![pk("id"), string("name"), int("age")])
            .build();

        let people_table_id = database
            .get_table_id(&SchemaObjectName::new("people", None))
            .unwrap();

        let name_col_id = database.get_column_id(people_table_id, "name").unwrap();
        let age_col_id = database.get_column_id(people_table_id, "age").unwrap();

        let name_value_col = Column::Param(SQLParamContainer::str("foo"));
        let age_value_col = Column::Param(SQLParamContainer::i32(5));

        let name_predicate =
            ConcretePredicate::Eq(Column::physical(name_col_id, None), name_value_col);
        let age_predicate =
            ConcretePredicate::Eq(Column::physical(age_col_id, None), age_value_col);

        let predicate = ConcretePredicate::And(Box::new(name_predicate), Box::new(age_predicate));

        assert_binding!(
            predicate.to_sql(&database),
            r#"("people"."name" = $1 AND "people"."age" = $2)"#,
            "foo",
            5
        );
    }

    #[multiplatform_test]
    fn string_predicates() {
        let database = DatabaseBuilder::new()
            .table("videos", vec![pk("id"), string("title")])
            .build();

        let table_id = database
            .get_table_id(&SchemaObjectName::new("videos", None))
            .unwrap();

        let title_col_id = database.get_column_id(table_id, "title").unwrap();

        fn title_test_data(title_col_id: ColumnId) -> (Column, Column) {
            let title_col = Column::physical(title_col_id, None);
            let title_value_col = Column::Param(SQLParamContainer::str("utawaku"));

            (title_col, title_value_col)
        }

        // like
        let (title_col, title_value_col) = title_test_data(title_col_id);

        let like_predicate =
            ConcretePredicate::StringLike(title_col, title_value_col, CaseSensitivity::Sensitive);
        assert_binding!(
            like_predicate.to_sql(&database),
            r#""videos"."title" LIKE $1"#,
            "utawaku"
        );

        // ilike
        let (title_col, title_value_col) = title_test_data(title_col_id);

        let ilike_predicate =
            ConcretePredicate::StringLike(title_col, title_value_col, CaseSensitivity::Insensitive);
        assert_binding!(
            ilike_predicate.to_sql(&database),
            r#""videos"."title" ILIKE $1"#,
            "utawaku"
        );

        // startsWith
        let (title_col, title_value_col) = title_test_data(title_col_id);

        let starts_with_predicate = ConcretePredicate::StringStartsWith(title_col, title_value_col);
        assert_binding!(
            starts_with_predicate.to_sql(&database),
            r#""videos"."title" LIKE $1 || '%'"#,
            "utawaku"
        );

        // endsWith
        let (title_col, title_value_col) = title_test_data(title_col_id);

        let ends_with_predicate = ConcretePredicate::StringEndsWith(title_col, title_value_col);
        assert_binding!(
            ends_with_predicate.to_sql(&database),
            r#""videos"."title" LIKE '%' || $1"#,
            "utawaku"
        );
    }

    #[multiplatform_test]
    fn json_predicates() {
        let database = DatabaseBuilder::new()
            .table("card", vec![pk("id"), json("data")])
            .build();

        let table_id = database
            .get_table_id(&SchemaObjectName::new("card", None))
            .unwrap();

        let json_col_id = database.get_column_id(table_id, "data").unwrap();

        fn json_test_data(json_col_id: ColumnId) -> (Column, Arc<serde_json::Value>, Column) {
            let json_col = Column::physical(json_col_id, None);

            let json_value: serde_json::Value = serde_json::from_str(
                r#"
                {
                    "a": 1,
                    "b": 2,
                    "c": 3
                }
                "#,
            )
            .unwrap();
            let json_value_col = Column::Param(SQLParamContainer::json(json_value.clone()));

            (json_col, Arc::new(json_value), json_value_col)
        }

        let json_key_list: serde_json::Value = serde_json::from_str(r#"["a", "b"]"#).unwrap();

        let json_key_col = Column::Param(SQLParamContainer::str("a"));

        //// Test bindings starting now

        // contains
        let (json_col, json_value, json_value_col) = json_test_data(json_col_id);

        let contains_predicate = ConcretePredicate::JsonContains(json_col, json_value_col);
        assert_binding!(
            contains_predicate.to_sql(&database),
            r#""card"."data" @> $1"#,
            *json_value
        );

        // containedBy
        let (json_col, json_value, json_value_col) = json_test_data(json_col_id);

        let contained_by_predicate = ConcretePredicate::JsonContainedBy(json_col, json_value_col);
        assert_binding!(
            contained_by_predicate.to_sql(&database),
            r#""card"."data" <@ $1"#,
            *json_value
        );

        // matchKey
        let json_key_list_col = Column::Param(SQLParamContainer::json(json_key_list.clone()));

        let (json_col, _, _) = json_test_data(json_col_id);

        let match_key_predicate = ConcretePredicate::JsonMatchKey(json_col, json_key_col);
        assert_binding!(
            match_key_predicate.to_sql(&database),
            r#""card"."data" ? $1"#,
            "a"
        );

        // matchAnyKey
        let (json_col, _, _) = json_test_data(json_col_id);

        let match_any_key_predicate =
            ConcretePredicate::JsonMatchAnyKey(json_col, json_key_list_col);
        assert_binding!(
            match_any_key_predicate.to_sql(&database),
            r#""card"."data" ?| $1"#,
            json_key_list
        );

        // matchAllKeys
        let json_key_list_col = Column::Param(SQLParamContainer::json(json_key_list.clone()));

        let (json_col, _, _) = json_test_data(json_col_id);

        let match_all_keys_predicate =
            ConcretePredicate::JsonMatchAllKeys(json_col, json_key_list_col);
        assert_binding!(
            match_all_keys_predicate.to_sql(&database),
            r#""card"."data" ?& $1"#,
            json_key_list
        );
    }
}
