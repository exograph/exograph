use super::{column::Column, Expression, ExpressionContext, ParameterBinding};

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
    Like(&'a Column<'a>, &'a Column<'a>),
}

impl<'a> Predicate<'a> {
    pub fn from_name(op_name: &str, lhs: &'a Column<'a>, rhs: &'a Column<'a>) -> Predicate<'a> {
        match op_name {
            "eq" => Predicate::Eq(lhs, &rhs),
            "lt" => Predicate::Lt(lhs, &rhs),
            "gt" => Predicate::Gt(lhs, &rhs),
            "like" => Predicate::Like(lhs, &rhs),
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
                        |stmt1, stmt2| format!("{} AND {}", stmt1, stmt2),
                    ),
                }
            }
            Predicate::Or(predicate1, predicate2) => combine(
                predicate1,
                predicate2,
                expression_context,
                |stmt1, stmt2| format!("{} OR {}", stmt1, stmt2),
            ),
<<<<<<< HEAD
            Predicate::Not(predicate) => {
                let expr = predicate.binding(expression_context);
                ParameterBinding::new(format!("NOT {}", expr.stmt), expr.params)
=======
            Predicate::Like(column1, column2) => {
                combine(*column1, *column2, expression_context, |stmt1, stmt2| {
                    format!("{} LIKE {}", stmt1, stmt2)
                })
>>>>>>> 0fa7a76 (Add support for the 'like' predicate for String fields in queries)
            }
        }
    }
}

fn combine<'a, E1: Expression, E2: Expression>(
    predicate1: &'a E1,
    predicate2: &'a E2,
    expression_context: &mut ExpressionContext,
    joiner: fn(String, String) -> String,
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
            references: None,
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
            typ: PhysicalColumnType::String,
            is_pk: false,
            is_autoincrement: false,
            references: None,
        };
        let name_col = Column::Physical(&name_col);
        let name_value_col = Column::Literal(Box::new("foo"));

        let age_col = PhysicalColumn {
            table_name: "people".to_string(),
            column_name: "age".to_string(),
            typ: PhysicalColumnType::Int { bits: IntBits::_16 },
            is_pk: false,
            is_autoincrement: false,
            references: None,
        };
        let age_col = Column::Physical(&age_col);
        let age_value_col = Column::Literal(Box::new(5));

        let name_predicate = Predicate::Eq(&name_col, &name_value_col);
        let age_predicate = Predicate::Eq(&age_col, &age_value_col);

        let predicate = Predicate::And(Box::new(name_predicate), Box::new(age_predicate));

        let mut expression_context = ExpressionContext::default();
        assert_binding!(
            &predicate.binding(&mut expression_context),
            r#""people"."name" = $1 AND "people"."age" = $2"#,
            "foo",
            5
        );
        assert_params!(predicate.binding(&mut expression_context).params, "foo", 5);
    }
}
