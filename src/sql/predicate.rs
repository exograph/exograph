use super::{column::Column, Expression, ExpressionContext, ParameterBinding};

#[derive(Debug)]
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
    use crate::sql::*;

    use super::*;

    #[test]
    fn true_predicate() {
        let mut expression_context = ExpressionContext::new();

        assert_binding!(&Predicate::True.binding(&mut expression_context), "true");
    }

    #[test]
    fn false_predicate() {
        let mut expression_context = ExpressionContext::new();
        assert_binding!(&Predicate::False.binding(&mut expression_context), "false");
    }

    #[test]
    fn eq_predicate() {
        let table_name = "people";

        let age_col = Column::Physical {
            table_name: table_name.to_string(),
            column_name: "age".to_string(),
        };
        let age_value_col = Column::Literal(Box::new(5));

        let predicate = Predicate::Eq(&age_col, &age_value_col);

        let mut expression_context = ExpressionContext::new();
        assert_binding!(
            &predicate.binding(&mut expression_context),
            r#""people"."age" = $1"#,
            5
        );
    }

    #[test]
    fn and_predicate() {
        let table_name = "people";

        let name_col = Column::Physical {
            table_name: table_name.to_string(),
            column_name: "name".to_string(),
        };
        let name_value_col = Column::Literal(Box::new("foo"));

        let predicate1 = Predicate::Eq(&name_col, &name_value_col);

        let age_col = Column::Physical {
            table_name: table_name.to_string(),
            column_name: "age".to_string(),
        };
        let age_value_col = Column::Literal(Box::new(5));

        let predicate2 = Predicate::Eq(&age_col, &age_value_col);

        let predicate = Predicate::And(Box::new(predicate1), Box::new(predicate2));

        let mut expression_context = ExpressionContext::new();
        assert_binding!(
            &predicate.binding(&mut expression_context),
            r#""people"."name" = $1 AND "people"."age" = $2"#,
            "foo",
            5
        );
        assert_params!(predicate.binding(&mut expression_context).params, "foo", 5);
    }
}
