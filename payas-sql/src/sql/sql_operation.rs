use super::{
    cte::Cte,
    insert::{DynamicInsert, Insert},
    select::Select,
    update::Update,
    Delete, Expression, ExpressionContext, OperationExpression, ParameterBinding,
};

pub enum SQLOperation<'a> {
    Select(Select<'a>),
    Insert(Insert<'a>),
    Delete(Delete<'a>),
    Update(Update<'a>),
    Cte(Cte<'a>),
}

impl<'a> OperationExpression for SQLOperation<'a> {
    fn binding(&self, expression_context: &mut ExpressionContext) -> ParameterBinding {
        match self {
            SQLOperation::Select(select) => select.binding(expression_context),
            SQLOperation::Insert(insert) => insert.binding(expression_context),
            SQLOperation::Delete(delete) => delete.binding(expression_context),
            SQLOperation::Update(update) => update.binding(expression_context),
            SQLOperation::Cte(cte) => cte.binding(expression_context),
        }
    }
}

pub enum DynamicOperation<'a, T> {
    // Select(DynamicSelect),
    Insert(DynamicInsert<'a, T>),
    // Update(DynamicUpdate),
    // Delete(DynamicDelete),
}
