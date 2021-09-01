use super::{
    cte::Cte,
    insert::{Insert, TemplateInsert},
    select::Select,
    transaction::TransactionStep,
    update::Update,
    Delete, Expression, ExpressionContext, OperationExpression, ParameterBinding,
};

#[derive(Debug)]
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

#[derive(Debug)]
pub enum TemplateSQLOperation<'a> {
    // Select(Select<'a>),
    Insert(TemplateInsert<'a>),
    // Delete(Delete<'a>),
    // Update(Update<'a>),
    // Cte(Cte<'a>),
}

impl<'a> TemplateSQLOperation<'a> {
    pub fn resolve(&self, prev_step: &'a TransactionStep<'a>) -> SQLOperation<'a> {
        match self {
            TemplateSQLOperation::Insert(insert) => SQLOperation::Insert(insert.resolve(prev_step)),
        }
    }
}
