use super::{
    cte::CTE, insert::Insert, select::Select, Expression, ExpressionContext, ParameterBinding,
};

pub enum SQLOperation<'a> {
    Select(Select<'a>),
    Insert(Insert<'a>),
    CTE(CTE<'a>),
}

impl<'a> Expression for SQLOperation<'a> {
    fn binding(&self, expression_context: &mut ExpressionContext) -> ParameterBinding {
        match self {
            SQLOperation::Select(select) => select.binding(expression_context),
            SQLOperation::CTE(cte) => cte.binding(expression_context),
            SQLOperation::Insert(insert) => insert.binding(expression_context),
        }
    }
}
