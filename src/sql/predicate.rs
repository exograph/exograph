use super::{column::Column, Expression, ParameterBinding};

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
    fn binding(&self) -> ParameterBinding {
        match &self {
            Predicate::True => ParameterBinding::new("1 = 1".to_string(), vec![]),
            Predicate::False => ParameterBinding::new("1 <> 1".to_string(), vec![]),
            Predicate::Eq(column1, column2) => combine(*column1, *column2, |stmt1, stmt2| {
                format!("{} = {}", stmt1, stmt2)
            }),
            Predicate::Neq(column1, column2) => combine(*column1, *column2, |stmt1, stmt2| {
                format!("{} <> {}", stmt1, stmt2)
            }),
            Predicate::Lt(column1, column2) => combine(*column1, *column2, |stmt1, stmt2| {
                format!("{} < {}", stmt1, stmt2)
            }),
            Predicate::Lte(column1, column2) => combine(*column1, *column2, |stmt1, stmt2| {
                format!("{} <= {}", stmt1, stmt2)
            }),
            Predicate::Gt(column1, column2) => combine(*column1, *column2, |stmt1, stmt2| {
                format!("{} > {}", stmt1, stmt2)
            }),
            Predicate::Gte(column1, column2) => combine(*column1, *column2, |stmt1, stmt2| {
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

fn combine<'a, E1: Expression, E2: Expression>(
    predicate1: &'a E1,
    predicate2: &'a E2,
    joiner: fn(String, String) -> String,
) -> ParameterBinding<'a> {
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

        let age_col = table.get_column("age").map(|c| Column::Physical(c)).unwrap();
        let age_value_col = Column::Literal(Box::new(5));

        let predicate = Predicate::Eq(
            &age_col,
            &age_value_col,
        );

        assert_binding!(&predicate.binding(), "people.age = ?", 5);
    }

    #[test]
    fn and_predicate() {
        let db = test_database();
        let table = db.get_table("people").unwrap();

        let name_col = Column::Physical(table.get_column("name").unwrap());
        let name_value_col = Column::Literal(Box::new("foo"));

        let predicate1 = Predicate::Eq(
            &name_col,
            &name_value_col,
        );

        let age_col = table.get_column("age").map(|c| Column::Physical(c)).unwrap();
        let age_value_col = Column::Literal(Box::new(5));

        let predicate2 = Predicate::Eq(
            &age_col,
            &age_value_col,
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
