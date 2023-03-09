use super::{column::Column, ExpressionBuilder, SQLBuilder};

/// Case sensitivity for string predicates.
#[derive(Debug, Clone, PartialEq, Eq, Copy)]
pub enum CaseSensitivity {
    Sensitive,
    Insensitive,
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

    // Prefer Predicate::and(), which simplifies the clause
    And(Box<Predicate<C>>, Box<Predicate<C>>),
    // Prefer Predicate::or(), which simplifies the clause
    Or(Box<Predicate<C>>, Box<Predicate<C>>),
    // Prefer Predicate::not(), which simplifies the clause
    Not(Box<Predicate<C>>),
}

pub type ConcretePredicate<'a> = Predicate<Column<'a>>;

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
            (lhs, rhs) => Predicate::And(Box::new(lhs), Box::new(rhs)),
        }
    }

    /// Logical or of two predicates, reducing to a simpler predicate if possible.
    pub fn or(lhs: Predicate<C>, rhs: Predicate<C>) -> Predicate<C> {
        match (lhs, rhs) {
            (Predicate::True, _) | (_, Predicate::True) => Predicate::True,
            (Predicate::False, rhs) => rhs,
            (lhs, Predicate::False) => lhs,
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

impl ParamEquality for Column<'_> {
    fn param_eq(&self, other: &Self) -> Option<bool> {
        match (self, other) {
            (Column::Param(v1), Column::Param(v2)) => Some(v1 == v2),
            _ => None,
        }
    }
}

impl<'a> ExpressionBuilder for ConcretePredicate<'a> {
    /// Build a predicate into a SQL string.
    fn build(&self, builder: &mut SQLBuilder) {
        match &self {
            ConcretePredicate::True => builder.push_str("TRUE"),
            ConcretePredicate::False => builder.push_str("FALSE"),
            ConcretePredicate::Eq(column1, column2) => {
                relational_combine(column1, column2, "=", builder)
            }
            ConcretePredicate::Neq(column1, column2) => {
                relational_combine(column1, column2, "<>", builder)
            }
            ConcretePredicate::Lt(column1, column2) => {
                relational_combine(column1, column2, "<", builder)
            }
            ConcretePredicate::Lte(column1, column2) => {
                relational_combine(column1, column2, "<=", builder)
            }
            ConcretePredicate::Gt(column1, column2) => {
                relational_combine(column1, column2, ">", builder)
            }
            ConcretePredicate::Gte(column1, column2) => {
                relational_combine(column1, column2, ">=", builder)
            }
            ConcretePredicate::In(column1, column2) => {
                relational_combine(column1, column2, "IN", builder)
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
                    builder,
                )
            }
            // we use the postgres concat operator (||) in order to handle both literals and column references
            ConcretePredicate::StringStartsWith(column1, column2) => {
                column1.build(builder);
                builder.push_str(" LIKE ");
                column2.build(builder);
                builder.push_str(" || '%'");
            }
            ConcretePredicate::StringEndsWith(column1, column2) => {
                column1.build(builder);
                builder.push_str(" LIKE '%' || ");
                column2.build(builder);
            }
            ConcretePredicate::JsonContains(column1, column2) => {
                relational_combine(column1, column2, "@>", builder)
            }
            ConcretePredicate::JsonContainedBy(column1, column2) => {
                relational_combine(column1, column2, "<@", builder)
            }
            ConcretePredicate::JsonMatchKey(column1, column2) => {
                relational_combine(column1, column2, "?", builder)
            }
            ConcretePredicate::JsonMatchAnyKey(column1, column2) => {
                relational_combine(column1, column2, "?|", builder)
            }
            ConcretePredicate::JsonMatchAllKeys(column1, column2) => {
                relational_combine(column1, column2, "?&", builder)
            }
            ConcretePredicate::And(predicate1, predicate2) => {
                logical_combine(predicate1, predicate2, "AND", builder)
            }
            ConcretePredicate::Or(predicate1, predicate2) => {
                logical_combine(predicate1, predicate2, "OR", builder)
            }
            ConcretePredicate::Not(predicate) => {
                builder.push_str("NOT(");
                predicate.build(builder);
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
    builder: &mut SQLBuilder,
) {
    left.build(builder);
    builder.push_space();
    builder.push_str(op);
    builder.push_space();
    right.build(builder);
}

/// Combine two expressions with a logical binary operator.
fn logical_combine<'a, E1: ExpressionBuilder, E2: ExpressionBuilder>(
    left: &'a E1,
    right: &'a E2,
    op: &'static str,
    builder: &mut SQLBuilder,
) {
    builder.push('(');
    left.build(builder);
    builder.push_space();
    builder.push_str(op);
    builder.push_space();
    right.build(builder);
    builder.push(')');
}

#[cfg(test)]
#[macro_use]
mod tests {
    use std::sync::Arc;

    use crate::sql::{
        physical_column::{IntBits, PhysicalColumn, PhysicalColumnType},
        SQLParamContainer,
    };

    use super::*;

    #[test]
    fn true_predicate() {
        assert_binding!(ConcretePredicate::True.into_sql(), "TRUE");
    }

    #[test]
    fn false_predicate() {
        assert_binding!(ConcretePredicate::False.into_sql(), "FALSE");
    }

    #[test]
    fn eq_predicate() {
        let age_col = PhysicalColumn {
            table_name: "people".to_string(),
            name: "age".to_string(),
            typ: PhysicalColumnType::Int { bits: IntBits::_16 },
            ..Default::default()
        };
        let age_col = Column::Physical(&age_col);
        let age_value_col = Column::Param(SQLParamContainer::new(5));

        let predicate = Predicate::Eq(age_col, age_value_col);

        assert_binding!(predicate.into_sql(), r#""people"."age" = $1"#, 5);
    }

    #[test]
    fn and_predicate() {
        let name_col = PhysicalColumn {
            table_name: "people".to_string(),
            name: "name".to_string(),
            typ: PhysicalColumnType::String { length: None },
            ..Default::default()
        };
        let name_col = Column::Physical(&name_col);
        let name_value_col = Column::Param(SQLParamContainer::new("foo"));

        let age_col = PhysicalColumn {
            table_name: "people".to_string(),
            name: "age".to_string(),
            typ: PhysicalColumnType::Int { bits: IntBits::_16 },
            ..Default::default()
        };
        let age_col = Column::Physical(&age_col);
        let age_value_col = Column::Param(SQLParamContainer::new(5));

        let name_predicate = ConcretePredicate::Eq(name_col, name_value_col);
        let age_predicate = ConcretePredicate::Eq(age_col, age_value_col);

        let predicate = ConcretePredicate::And(Box::new(name_predicate), Box::new(age_predicate));

        assert_binding!(
            predicate.into_sql(),
            r#"("people"."name" = $1 AND "people"."age" = $2)"#,
            "foo",
            5
        );
    }

    #[test]
    fn string_predicates() {
        let title_physical_col = PhysicalColumn {
            table_name: "videos".to_string(),
            name: "title".to_string(),
            typ: PhysicalColumnType::String { length: None },
            ..Default::default()
        };

        fn title_test_data(title_physical_col: &PhysicalColumn) -> (Column<'_>, Column<'_>) {
            let title_col = Column::Physical(title_physical_col);
            let title_value_col = Column::Param(SQLParamContainer::new("utawaku"));

            (title_col, title_value_col)
        }

        // like
        let (title_col, title_value_col) = title_test_data(&title_physical_col);

        let like_predicate =
            ConcretePredicate::StringLike(title_col, title_value_col, CaseSensitivity::Sensitive);
        assert_binding!(
            like_predicate.into_sql(),
            r#""videos"."title" LIKE $1"#,
            "utawaku"
        );

        // ilike
        let (title_col, title_value_col) = title_test_data(&title_physical_col);

        let ilike_predicate =
            ConcretePredicate::StringLike(title_col, title_value_col, CaseSensitivity::Insensitive);
        assert_binding!(
            ilike_predicate.into_sql(),
            r#""videos"."title" ILIKE $1"#,
            "utawaku"
        );

        // startsWith
        let (title_col, title_value_col) = title_test_data(&title_physical_col);

        let starts_with_predicate = ConcretePredicate::StringStartsWith(title_col, title_value_col);
        assert_binding!(
            starts_with_predicate.into_sql(),
            r#""videos"."title" LIKE $1 || '%'"#,
            "utawaku"
        );

        // endsWith
        let (title_col, title_value_col) = title_test_data(&title_physical_col);

        let ends_with_predicate = ConcretePredicate::StringEndsWith(title_col, title_value_col);
        assert_binding!(
            ends_with_predicate.into_sql(),
            r#""videos"."title" LIKE '%' || $1"#,
            "utawaku"
        );
    }

    #[test]
    fn json_predicates() {
        //// Setup

        let json_physical_col = PhysicalColumn {
            table_name: "card".to_string(),
            name: "data".to_string(),
            typ: PhysicalColumnType::Json,
            ..Default::default()
        };

        fn json_test_data(
            json_physical_col: &PhysicalColumn,
        ) -> (Column<'_>, Arc<serde_json::Value>, Column<'_>) {
            let json_col = Column::Physical(json_physical_col);

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
        let (json_col, json_value, json_value_col) = json_test_data(&json_physical_col);

        let contains_predicate = ConcretePredicate::JsonContains(json_col, json_value_col);
        assert_binding!(
            contains_predicate.into_sql(),
            r#""card"."data" @> $1"#,
            *json_value
        );

        // containedBy
        let (json_col, json_value, json_value_col) = json_test_data(&json_physical_col);

        let contained_by_predicate = ConcretePredicate::JsonContainedBy(json_col, json_value_col);
        assert_binding!(
            contained_by_predicate.into_sql(),
            r#""card"."data" <@ $1"#,
            *json_value
        );

        // matchKey
        let json_key_list_col = Column::Param(SQLParamContainer::new(json_key_list.clone()));

        let (json_col, _, _) = json_test_data(&json_physical_col);

        let match_key_predicate = ConcretePredicate::JsonMatchKey(json_col, json_key_col);
        assert_binding!(match_key_predicate.into_sql(), r#""card"."data" ? $1"#, "a");

        // matchAnyKey
        let (json_col, _, _) = json_test_data(&json_physical_col);

        let match_any_key_predicate =
            ConcretePredicate::JsonMatchAnyKey(json_col, json_key_list_col);
        assert_binding!(
            match_any_key_predicate.into_sql(),
            r#""card"."data" ?| $1"#,
            json_key_list
        );

        // matchAllKeys
        let json_key_list_col = Column::Param(SQLParamContainer::new(json_key_list.clone()));

        let (json_col, _, _) = json_test_data(&json_physical_col);

        let match_all_keys_predicate =
            ConcretePredicate::JsonMatchAllKeys(json_col, json_key_list_col);
        assert_binding!(
            match_all_keys_predicate.into_sql(),
            r#""card"."data" ?& $1"#,
            json_key_list
        );
    }
}
