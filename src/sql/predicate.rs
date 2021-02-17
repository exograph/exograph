use super::{column::Column, Expression, ParameterBinding};

#[derive(Debug)]
pub enum Predicate {
    True,
    False,
    Eq(Column, Column),
    Neq(Column, Column),
    Lt(Column, Column),
    Lte(Column, Column),
    Gt(Column, Column),
    Gte(Column, Column),
    And(Box<Predicate>, Box<Predicate>),
    Or(Box<Predicate>, Box<Predicate>),
}

impl Expression for Predicate {
    fn binding(&self) -> ParameterBinding {
        match &self {
            Predicate::True => ParameterBinding::new("1 = 1".to_string(), vec![]),
            Predicate::False => ParameterBinding::new("1 <> 1".to_string(), vec![]),
            Predicate::Eq(column1, column2) => combine(column1, column2, |stmt1, stmt2| {
                format!("{} = {}", stmt1, stmt2)
            }),
            Predicate::Neq(column1, column2) => combine(column1, column2, |stmt1, stmt2| {
                format!("{} <> {}", stmt1, stmt2)
            }),
            Predicate::Lt(column1, column2) => combine(column1, column2, |stmt1, stmt2| {
                format!("{} < {}", stmt1, stmt2)
            }),
            Predicate::Lte(column1, column2) => combine(column1, column2, |stmt1, stmt2| {
                format!("{} <= {}", stmt1, stmt2)
            }),
            Predicate::Gt(column1, column2) => combine(column1, column2, |stmt1, stmt2| {
                format!("{} > {}", stmt1, stmt2)
            }),
            Predicate::Gte(column1, column2) => combine(column1, column2, |stmt1, stmt2| {
                format!("{} >= {}", stmt1, stmt2)
            }),
            Predicate::And(predicate1, predicate2) => {
                combine(predicate1, predicate2, |stmt1, stmt2| {
                    format!("{} AND {}", stmt1, stmt2)
                })
            }
            Predicate::Or(predicate1, predicate2) => {
                combine(predicate1, predicate2, |stmt1, stmt2| {
                    format!("{} OR {}", stmt1, stmt2)
                })
            }
        }
    }
}

fn combine<E1: Expression, E2: Expression>(
    predicate1: &E1,
    predicate2: &E2,
    joiner: fn(String, String) -> String,
) -> ParameterBinding {
    let expr1 = predicate1.binding();
    let expr2 = predicate2.binding();
    let mut params = expr1.params;
    params.extend(expr2.params);
    ParameterBinding::new(joiner(expr1.stmt, expr2.stmt), params)
}

#[cfg(test)]
#[macro_use]
mod tests {
    use test_util::test_database;

    use crate::sql::*;
    use std::sync::Arc;

    use super::*;

    #[test]
    fn true_predicate() {
        assert_binding!(&Predicate::True.binding(), "1 = 1");
    }

    #[test]
    fn false_predicate() {
        assert_binding!(&Predicate::False.binding(), "1 <> 1");
    }

    #[test]
    fn eq_predicate() {
        let db = test_database();
        let table = db.get_table("people").unwrap();

        let predicate = Predicate::Eq(
            Column::Physical(table.get_column("age").unwrap()),
            Column::Literal(Arc::new(5)),
        );

        assert_binding!(&predicate.binding(), "people.age = ?", 5);
    }

    #[test]
    fn and_predicate() {
        let db = test_database();
        let table = db.get_table("people").unwrap();

        let predicate1 = Predicate::Eq(
            Column::Physical(table.get_column("name").unwrap()),
            Column::Literal(Arc::new("foo")),
        );
        let predicate2 = Predicate::Eq(
            Column::Physical(table.get_column("age").unwrap()),
            Column::Literal(Arc::new(5)),
        );

        let predicate = Predicate::And(Box::new(predicate1), Box::new(predicate2));

        assert_binding!(
            &predicate.binding(),
            "people.name = ? AND people.age = ?",
            "foo",
            5
        );
        assert_params!(predicate.binding().params, "foo", 5);
    }

 
}
