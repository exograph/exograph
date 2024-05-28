// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use crate::{Database, VectorDistanceFunction};

use super::{column::Column, vector::VectorDistance, ExpressionBuilder, SQLBuilder};

/// Case sensitivity for string predicates.
#[derive(Debug, Clone, PartialEq, Eq, Copy)]
pub enum CaseSensitivity {
    Sensitive,
    Insensitive,
}

#[derive(Debug, Clone, PartialEq, Eq, Copy)]
pub enum NumericComparator {
    Eq,
    Neq,
    Lt,
    Lte,
    Gt,
    Gte,
}

/// A predicate is a boolean expression that can be used in a WHERE clause.
#[derive(Debug, PartialEq, Clone)]
pub enum Predicate<C>
where
    C: PartialEq + ParamEquality,
{
    True,
    False,
    Eq(C, C),
    Neq(C, C),
    Lt(C, C),
    Lte(C, C),
    Gt(C, C),
    Gte(C, C),
    In(C, C),

    // string predicates
    StringLike(C, C, CaseSensitivity),
    StringStartsWith(C, C),
    StringEndsWith(C, C),

    // json predicates
    JsonContains(C, C),
    JsonContainedBy(C, C),
    JsonMatchKey(C, C),
    JsonMatchAnyKey(C, C),
    JsonMatchAllKeys(C, C),

    VectorDistance(C, C, VectorDistanceFunction, NumericComparator, C),

    // Prefer Predicate::and(), which simplifies the clause
    And(Box<Predicate<C>>, Box<Predicate<C>>),
    // Prefer Predicate::or(), which simplifies the clause
    Or(Box<Predicate<C>>, Box<Predicate<C>>),
    // Prefer Predicate::not(), which simplifies the clause
    Not(Box<Predicate<C>>),
}

pub type ConcretePredicate = Predicate<Column>;

impl<C> Predicate<C>
where
    C: PartialEq + ParamEquality,
{
    /// Compare two columns and reduce to a simpler predicate if possible.
    pub fn eq(lhs: C, rhs: C) -> Predicate<C> {
        if lhs == rhs {
            Predicate::True
        } else {
            // For literal columns, we can check for Predicate::False directly
            match lhs.param_eq(&rhs) {
                Some(false) => Predicate::False, // We don't need to check for `Some(true)`, since the above `lhs == rhs` check would have taken care of that
                _ => Predicate::Eq(lhs, rhs),
            }
        }
    }

    /// Compare two columns and reduce to a simpler predicate if possible
    pub fn neq(lhs: C, rhs: C) -> Predicate<C> {
        !Self::eq(lhs, rhs)
    }

    /// Logical and of two predicates, reducing to a simpler predicate if possible.
    pub fn and(lhs: Predicate<C>, rhs: Predicate<C>) -> Predicate<C> {
        match (lhs, rhs) {
            (Predicate::False, _) | (_, Predicate::False) => Predicate::False,
            (Predicate::True, rhs) => rhs,
            (lhs, Predicate::True) => lhs,
            (lhs, rhs) if lhs == rhs => lhs,
            (lhs, rhs) => Predicate::And(Box::new(lhs), Box::new(rhs)),
        }
    }

    /// Logical or of two predicates, reducing to a simpler predicate if possible.
    pub fn or(lhs: Predicate<C>, rhs: Predicate<C>) -> Predicate<C> {
        match (lhs, rhs) {
            (Predicate::True, _) | (_, Predicate::True) => Predicate::True,
            (Predicate::False, rhs) => rhs,
            (lhs, Predicate::False) => lhs,
            (lhs, rhs) if lhs == rhs => lhs,
            (lhs, rhs) => Predicate::Or(Box::new(lhs), Box::new(rhs)),
        }
    }
}

impl<C> From<bool> for Predicate<C>
where
    C: PartialEq + ParamEquality,
{
    fn from(b: bool) -> Predicate<C> {
        if b {
            Predicate::True
        } else {
            Predicate::False
        }
    }
}

impl<C> std::ops::Not for Predicate<C>
where
    C: PartialEq + ParamEquality,
{
    type Output = Predicate<C>;

    fn not(self) -> Self::Output {
        match self {
            // Reduced to a simpler form when possible, else fall back to Predicate::Not
            Predicate::True => Predicate::False,
            Predicate::False => Predicate::True,
            Predicate::Eq(lhs, rhs) => Predicate::Neq(lhs, rhs),
            Predicate::Neq(lhs, rhs) => Predicate::Eq(lhs, rhs),
            Predicate::Lt(lhs, rhs) => Predicate::Gte(lhs, rhs),
            Predicate::Lte(lhs, rhs) => Predicate::Gt(lhs, rhs),
            Predicate::Gt(lhs, rhs) => Predicate::Lte(lhs, rhs),
            Predicate::Gte(lhs, rhs) => Predicate::Lt(lhs, rhs),
            predicate => Predicate::Not(Box::new(predicate)),
        }
    }
}

/// Compare two parameters so that we can reduce a predicate to a boolean before passing it to
/// the database. With a simpler form, we may be able to skip passing it to the database completely. For
/// example, `Predicate::Eq(Column::Param(1), Column::Param(1))` can be reduced to
/// true.
pub trait ParamEquality {
    /// Returns `None` if one of the columns is not a parameter, otherwise returns `Some(true)` if
    /// the parameters are equal, and `Some(false)` if they are not.
    fn param_eq(&self, other: &Self) -> Option<bool>;
}

impl ParamEquality for Column {
    fn param_eq(&self, other: &Self) -> Option<bool> {
        match (self, other) {
            (Column::Param(v1), Column::Param(v2)) => Some(v1 == v2),
            _ => None,
        }
    }
}

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

    use crate::schema::table_spec::TableSpec;
    use crate::schema::test_helper::{int_column, json_column, pk_column, string_column};
    use crate::{schema::database_spec::DatabaseSpec, sql::SQLParamContainer};
    use crate::{ColumnId, PhysicalTableName};
    use multiplatform_test::multiplatform_test;

    use super::*;

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
        let database = DatabaseSpec::new(vec![TableSpec::new(
            PhysicalTableName::new("people", None),
            vec![pk_column("id"), int_column("age")],
            vec![],
        )])
        .to_database();

        let people_table_id = database
            .get_table_id(&PhysicalTableName::new("people", None))
            .unwrap();
        let age_column_id = database.get_column_id(people_table_id, "age").unwrap();

        let age_col = Column::physical(age_column_id, None);
        let age_value_col = Column::Param(SQLParamContainer::new(5, Type::INT4));

        let predicate = Predicate::Eq(age_col, age_value_col);

        assert_binding!(predicate.to_sql(&database), r#""people"."age" = $1"#, 5);
    }

    #[multiplatform_test]
    fn and_predicate() {
        let database = DatabaseSpec::new(vec![TableSpec::new(
            PhysicalTableName::new("people", None),
            vec![pk_column("id"), string_column("name"), int_column("age")],
            vec![],
        )])
        .to_database();

        let people_table_id = database
            .get_table_id(&PhysicalTableName::new("people", None))
            .unwrap();

        let name_col_id = database.get_column_id(people_table_id, "name").unwrap();
        let age_col_id = database.get_column_id(people_table_id, "age").unwrap();

        let name_value_col = Column::Param(SQLParamContainer::new("foo"));
        let age_value_col = Column::Param(SQLParamContainer::new(5));

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
        let database = DatabaseSpec::new(vec![TableSpec::new(
            PhysicalTableName::new("videos", None),
            vec![pk_column("id"), string_column("title")],
            vec![],
        )])
        .to_database();

        let table_id = database
            .get_table_id(&PhysicalTableName::new("videos", None))
            .unwrap();

        let title_col_id = database.get_column_id(table_id, "title").unwrap();

        fn title_test_data(title_col_id: ColumnId) -> (Column, Column) {
            let title_col = Column::physical(title_col_id, None);
            let title_value_col = Column::Param(SQLParamContainer::new("utawaku"));

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
        let database = DatabaseSpec::new(vec![TableSpec::new(
            PhysicalTableName::new("card", None),
            vec![pk_column("id"), json_column("data")],
            vec![],
        )])
        .to_database();

        let table_id = database
            .get_table_id(&PhysicalTableName::new("card", None))
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
            let json_value_col = Column::Param(SQLParamContainer::new(json_value.clone()));

            (json_col, Arc::new(json_value), json_value_col)
        }

        let json_key_list: serde_json::Value = serde_json::from_str(r#"["a", "b"]"#).unwrap();

        let json_key_col = Column::Param(SQLParamContainer::new("a"));

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
        let json_key_list_col = Column::Param(SQLParamContainer::new(json_key_list.clone()));

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
        let json_key_list_col = Column::Param(SQLParamContainer::new(json_key_list.clone()));

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
