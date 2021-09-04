use std::rc::Rc;

use super::{
    cte::Cte,
    delete::TemplateDelete,
    insert::{Insert, TemplateInsert},
    select::Select,
    transaction::TransactionStep,
    update::{TemplateUpdate, Update},
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
    Insert(TemplateInsert<'a>),
    Update(TemplateUpdate<'a>),
    Delete(TemplateDelete<'a>),
}

impl<'a> TemplateSQLOperation<'a> {
    pub fn resolve(&'a self, prev_step: Rc<TransactionStep<'a>>) -> Vec<SQLOperation<'a>> {
        match self {
            TemplateSQLOperation::Insert(insert) => insert
                .resolve(prev_step)
                .into_iter()
                .map(SQLOperation::Insert)
                .collect(),
            TemplateSQLOperation::Update(update) => update
                .resolve(prev_step)
                .into_iter()
                .map(SQLOperation::Update)
                .collect(),
            TemplateSQLOperation::Delete(delete) => {
                vec![SQLOperation::Delete(delete.resolve(prev_step))]
            }
        }
    }
}
