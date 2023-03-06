use crate::sql::OperationExpression;

use super::{select::Select, sql_operation::SQLOperation, Expression, ParameterBinding};

#[derive(Debug)]
pub struct Cte<'a> {
    pub expressions: Vec<CteExpression<'a>>,
    pub select: Select<'a>,
}

#[derive(Debug)]
pub struct CteExpression<'a> {
    pub name: String,
    pub operation: SQLOperation<'a>,
}

impl<'a> Expression for Cte<'a> {
    fn binding(&self) -> ParameterBinding {
        let exprs: Vec<_> = self
            .expressions
            .iter()
            .map(
                |CteExpression { name, operation }| ParameterBinding::CteExpression {
                    name: name.clone(),
                    operation: Box::new(operation.binding()),
                },
            )
            .collect();

        let select_expr = self.select.binding();

        ParameterBinding::Cte {
            exprs,
            select: Box::new(select_expr),
        }
    }
}
