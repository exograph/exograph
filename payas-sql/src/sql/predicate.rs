use super::{column::Column, Expression, ExpressionContext, ParameterBinding};

#[derive(Debug, Clone, PartialEq)]
pub enum CaseSensitivity {
    Sensitive,
    Insensitive,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Predicate<'a> {
    True,
    False,
    Eq(&'a Column<'a>, &'a Column<'a>),
    Neq(&'a Column<'a>, &'a Column<'a>),
    Lt(&'a Column<'a>, &'a Column<'a>),
    Lte(&'a Column<'a>, &'a Column<'a>),
    Gt(&'a Column<'a>, &'a Column<'a>),
    Gte(&'a Column<'a>, &'a Column<'a>),
    And(Box<Predicate<'a>>, Box<Predicate<'a>>),
    Or(Box<Predicate<'a>>, Box<Predicate<'a>>),
    Not(Box<Predicate<'a>>),

    // string predicates
    StringLike(&'a Column<'a>, &'a Column<'a>, CaseSensitivity),
    StringStartsWith(&'a Column<'a>, &'a Column<'a>),
    StringEndsWith(&'a Column<'a>, &'a Column<'a>),

    // json predicates
    JsonContains(&'a Column<'a>, &'a Column<'a>),
    JsonContainedBy(&'a Column<'a>, &'a Column<'a>),
    JsonMatchKey(&'a Column<'a>, &'a Column<'a>),
    JsonMatchAnyKey(&'a Column<'a>, &'a Column<'a>),
    JsonMatchAllKeys(&'a Column<'a>, &'a Column<'a>),
}

impl<'a> Predicate<'a> {
    pub fn from_name(op_name: &str, lhs: &'a Column<'a>, rhs: &'a Column<'a>) -> Predicate<'a> {
        match op_name {
            "eq" => Predicate::Eq(lhs, rhs),
            "neq" => Predicate::Neq(lhs, rhs),
            "lt" => Predicate::Lt(lhs, rhs),
            "lte" => Predicate::Lte(lhs, rhs),
            "gt" => Predicate::Gt(lhs, rhs),
            "gte" => Predicate::Gte(lhs, rhs),
            "like" => Predicate::StringLike(lhs, rhs, CaseSensitivity::Sensitive),
            "ilike" => Predicate::StringLike(lhs, rhs, CaseSensitivity::Insensitive),
            "startsWith" => Predicate::StringStartsWith(lhs, rhs),
            "endsWith" => Predicate::StringEndsWith(lhs, rhs),
            "contains" => Predicate::JsonContains(lhs, rhs),
            "containedBy" => Predicate::JsonContainedBy(lhs, rhs),
            "matchKey" => Predicate::JsonMatchKey(lhs, rhs),
            "matchAnyKey" => Predicate::JsonMatchAnyKey(lhs, rhs),
            "matchAllKeys" => Predicate::JsonMatchAllKeys(lhs, rhs),
            _ => todo!(),
        }
    }

    pub fn not(&self) -> Predicate<'a> {
        match self {
            Predicate::True => Predicate::False,
            Predicate::False => Predicate::True,
            predicate => Predicate::Not(Box::new(predicate.clone())),
        }
    }
}

impl<'a> Expression for Predicate<'a> {
    fn binding(&self, expression_context: &mut ExpressionContext) -> ParameterBinding {
        match &self {
            Predicate::True => ParameterBinding::new("true".to_string(), vec![]),
            Predicate::False => ParameterBinding::new("false".to_string(), vec![]),
            Predicate::Eq(column1, column2) => {
                combine(*column1, *column2, expression_context, |stmt1, stmt2| {
                    format!("{} = {}", stmt1, stmt2)
                })
            }
            Predicate::Neq(column1, column2) => {
                combine(*column1, *column2, expression_context, |stmt1, stmt2| {
                    format!("{} <> {}", stmt1, stmt2)
                })
            }
            Predicate::Lt(column1, column2) => {
                combine(*column1, *column2, expression_context, |stmt1, stmt2| {
                    format!("{} < {}", stmt1, stmt2)
                })
            }
            Predicate::Lte(column1, column2) => {
                combine(*column1, *column2, expression_context, |stmt1, stmt2| {
                    format!("{} <= {}", stmt1, stmt2)
                })
            }
            Predicate::Gt(column1, column2) => {
                combine(*column1, *column2, expression_context, |stmt1, stmt2| {
                    format!("{} > {}", stmt1, stmt2)
                })
            }
            Predicate::Gte(column1, column2) => {
                combine(*column1, *column2, expression_context, |stmt1, stmt2| {
                    format!("{} >= {}", stmt1, stmt2)
                })
            }
            Predicate::And(predicate1, predicate2) => {
                match (predicate1.as_ref(), predicate2.as_ref()) {
                    (Predicate::True, predicate) => predicate.binding(expression_context),
                    (Predicate::False, _) => Predicate::False.binding(expression_context),
                    (predicate, Predicate::True) => predicate.binding(expression_context),
                    (_, Predicate::False) => Predicate::False.binding(expression_context),
                    (predicate1, predicate2) => combine(
                        predicate1,
                        predicate2,
                        expression_context,
                        |stmt1, stmt2| format!("({} AND {})", stmt1, stmt2),
                    ),
                }
            }
            Predicate::Or(predicate1, predicate2) => combine(
                predicate1,
                predicate2,
                expression_context,
                |stmt1, stmt2| format!("({} OR {})", stmt1, stmt2),
            ),
            Predicate::Not(predicate) => {
                let expr = predicate.binding(expression_context);
                ParameterBinding::new(format!("NOT ({})", expr.stmt), expr.params)
            }
            Predicate::StringLike(column1, column2, case_sensitivity) => {
                combine(*column1, *column2, expression_context, |stmt1, stmt2| {
                    if *case_sensitivity == CaseSensitivity::Insensitive {
                        format!("{} ILIKE {}", stmt1, stmt2)
                    } else {
                        format!("{} LIKE {}", stmt1, stmt2)
                    }
                })
            }
            // we use the postgres concat operator (||) in order to handle both literals
            // and column references
            Predicate::StringStartsWith(column1, column2) => {
                combine(*column1, *column2, expression_context, |stmt1, stmt2| {
                    format!("{} LIKE {} || '%'", stmt1, stmt2)
                })
            }
            Predicate::StringEndsWith(column1, column2) => {
                combine(*column1, *column2, expression_context, |stmt1, stmt2| {
                    format!("{} LIKE '%' || {}", stmt1, stmt2)
                })
            }
            Predicate::JsonContains(column1, column2) => {
                combine(*column1, *column2, expression_context, |stmt1, stmt2| {
                    format!("{} @> {}", stmt1, stmt2)
                })
            }
            Predicate::JsonContainedBy(column1, column2) => {
                combine(*column1, *column2, expression_context, |stmt1, stmt2| {
                    format!("{} <@ {}", stmt1, stmt2)
                })
            }
            Predicate::JsonMatchKey(column1, column2) => {
                combine(*column1, *column2, expression_context, |stmt1, stmt2| {
                    format!("{} ? {}", stmt1, stmt2)
                })
            }
            Predicate::JsonMatchAnyKey(column1, column2) => {
                combine(*column1, *column2, expression_context, |stmt1, stmt2| {
                    format!("{} ?| {}", stmt1, stmt2)
                })
            }
            Predicate::JsonMatchAllKeys(column1, column2) => {
                combine(*column1, *column2, expression_context, |stmt1, stmt2| {
                    format!("{} ?& {}", stmt1, stmt2)
                })
            }
        }
    }
}

fn combine<'a, E1: Expression, E2: Expression>(
    predicate1: &'a E1,
    predicate2: &'a E2,
    expression_context: &mut ExpressionContext,
    joiner: impl Fn(String, String) -> String,
) -> ParameterBinding<'a> {
    let expr1 = predicate1.binding(expression_context);
    let expr2 = predicate2.binding(expression_context);
    let mut params = expr1.params;
    params.extend(expr2.params);
    ParameterBinding::new(joiner(expr1.stmt, expr2.stmt), params)
}

#[cfg(test)]
#[macro_use]
mod tests {
    use crate::sql::column::{IntBits, PhysicalColumn, PhysicalColumnType};

    use super::*;

    #[test]
    fn true_predicate() {
        let mut expression_context = ExpressionContext::default();

        assert_binding!(&Predicate::True.binding(&mut expression_context), "true");
    }

    #[test]
    fn false_predicate() {
        let mut expression_context = ExpressionContext::default();
        assert_binding!(&Predicate::False.binding(&mut expression_context), "false");
    }

    #[test]
    fn eq_predicate() {
        let age_col = PhysicalColumn {
            table_name: "people".to_string(),
            column_name: "age".to_string(),
            typ: PhysicalColumnType::Int { bits: IntBits::_16 },
            is_pk: false,
            is_autoincrement: false,
        };
        let age_col = Column::Physical(&age_col);
        let age_value_col = Column::Literal(Box::new(5));

        let predicate = Predicate::Eq(&age_col, &age_value_col);

        let mut expression_context = ExpressionContext::default();
        assert_binding!(
            &predicate.binding(&mut expression_context),
            r#""people"."age" = $1"#,
            5
        );
    }

    #[test]
    fn and_predicate() {
        let name_col = PhysicalColumn {
            table_name: "people".to_string(),
            column_name: "name".to_string(),
            typ: PhysicalColumnType::String { length: None },
            is_pk: false,
            is_autoincrement: false,
        };
        let name_col = Column::Physical(&name_col);
        let name_value_col = Column::Literal(Box::new("foo"));

        let age_col = PhysicalColumn {
            table_name: "people".to_string(),
            column_name: "age".to_string(),
            typ: PhysicalColumnType::Int { bits: IntBits::_16 },
            is_pk: false,
            is_autoincrement: false,
        };
        let age_col = Column::Physical(&age_col);
        let age_value_col = Column::Literal(Box::new(5));

        let name_predicate = Predicate::Eq(&name_col, &name_value_col);
        let age_predicate = Predicate::Eq(&age_col, &age_value_col);

        let predicate = Predicate::And(Box::new(name_predicate), Box::new(age_predicate));

        let mut expression_context = ExpressionContext::default();
        assert_binding!(
            &predicate.binding(&mut expression_context),
            r#"("people"."name" = $1 AND "people"."age" = $2)"#,
            "foo",
            5
        );
        assert_params!(predicate.binding(&mut expression_context).params, "foo", 5);
    }

    #[test]
    fn string_predicates() {
        let title_col = PhysicalColumn {
            table_name: "videos".to_string(),
            column_name: "title".to_string(),
            typ: PhysicalColumnType::String { length: None },
            is_pk: false,
            is_autoincrement: false,
        };
        let title_col = Column::Physical(&title_col);
        let title_value_col = Column::Literal(Box::new("utawaku"));

        // like
        let mut expression_context = ExpressionContext::default();
        let like_predicate =
            Predicate::StringLike(&title_col, &title_value_col, CaseSensitivity::Sensitive);
        assert_binding!(
            &like_predicate.binding(&mut expression_context),
            r#""videos"."title" LIKE $1"#,
            "utawaku"
        );

        // ilike
        let mut expression_context = ExpressionContext::default();
        let ilike_predicate =
            Predicate::StringLike(&title_col, &title_value_col, CaseSensitivity::Insensitive);
        assert_binding!(
            &ilike_predicate.binding(&mut expression_context),
            r#""videos"."title" ILIKE $1"#,
            "utawaku"
        );

        // startsWith
        let mut expression_context = ExpressionContext::default();
        let starts_with_predicate = Predicate::StringStartsWith(&title_col, &title_value_col);
        assert_binding!(
            &starts_with_predicate.binding(&mut expression_context),
            r#""videos"."title" LIKE $1 || '%'"#,
            "utawaku"
        );

        // endsWith
        let mut expression_context = ExpressionContext::default();
        let ends_with_predicate = Predicate::StringEndsWith(&title_col, &title_value_col);
        assert_binding!(
            &ends_with_predicate.binding(&mut expression_context),
            r#""videos"."title" LIKE '%' || $1"#,
            "utawaku"
        );
    }

    #[test]
    fn json_predicates() {
        //// Setup

        let json_col = PhysicalColumn {
            table_name: "card".to_string(),
            column_name: "data".to_string(),
            typ: PhysicalColumnType::Json,
            is_pk: false,
            is_autoincrement: false,
        };
        let json_col = Column::Physical(&json_col);

        let json_value: Box<serde_json::Value> = Box::new(
            serde_json::from_str(
                r#"
            {
                "a": 1,
                "b": 2,
                "c": 3
            }
            "#,
            )
            .unwrap(),
        );
        let json_value_col = Column::Literal(json_value.clone());

        let json_key_list: Box<serde_json::Value> =
            Box::new(serde_json::from_str(r#"["a", "b"]"#).unwrap());
        let json_key_list_col = Column::Literal(json_key_list.clone());

        let json_key_col = Column::Literal(Box::new("a"));

        //// Test bindings starting now

        // contains
        let mut expression_context = ExpressionContext::default();
        let contains_predicate = Predicate::JsonContains(&json_col, &json_value_col);
        assert_binding!(
            &contains_predicate.binding(&mut expression_context),
            r#""card"."data" @> $1"#,
            *json_value
        );

        // containedBy
        let mut expression_context = ExpressionContext::default();
        let contained_by_predicate = Predicate::JsonContainedBy(&json_col, &json_value_col);
        assert_binding!(
            &contained_by_predicate.binding(&mut expression_context),
            r#""card"."data" <@ $1"#,
            *json_value
        );

        // matchKey
        let mut expression_context = ExpressionContext::default();
        let match_key_predicate = Predicate::JsonMatchKey(&json_col, &json_key_col);
        assert_binding!(
            &match_key_predicate.binding(&mut expression_context),
            r#""card"."data" ? $1"#,
            "a"
        );

        // matchAnyKey
        let mut expression_context = ExpressionContext::default();
        let match_any_key_predicate = Predicate::JsonMatchAnyKey(&json_col, &json_key_list_col);
        assert_binding!(
            &match_any_key_predicate.binding(&mut expression_context),
            r#""card"."data" ?| $1"#,
            *json_key_list
        );

        // matchAllKeys
        let mut expression_context = ExpressionContext::default();
        let match_all_keys_predicate = Predicate::JsonMatchAllKeys(&json_col, &json_key_list_col);
        assert_binding!(
            &match_all_keys_predicate.binding(&mut expression_context),
            r#""card"."data" ?& $1"#,
            *json_key_list
        );
    }
}
