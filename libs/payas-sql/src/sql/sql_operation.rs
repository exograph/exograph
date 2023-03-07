use super::{
    cte::Cte,
    delete::Delete,
    delete::TemplateDelete,
    insert::{Insert, TemplateInsert},
    select::Select,
    transaction::{TransactionContext, TransactionStepId},
    update::{TemplateUpdate, Update},
    ExpressionBuilder, SQLBuilder,
};

#[derive(Debug)]
pub enum SQLOperation<'a> {
    Select(Select<'a>),
    Insert(Insert<'a>),
    Delete(Delete<'a>),
    Update(Update<'a>),
    Cte(Cte<'a>),
}

impl<'a> ExpressionBuilder for SQLOperation<'a> {
    fn build(&self, builder: &mut SQLBuilder) {
        match self {
            SQLOperation::Select(select) => select.build(builder),
            SQLOperation::Insert(insert) => insert.build(builder),
            SQLOperation::Delete(delete) => delete.build(builder),
            SQLOperation::Update(update) => update.build(builder),
            SQLOperation::Cte(cte) => cte.build(builder),
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
    pub fn resolve(
        &'a self,
        prev_step_id: TransactionStepId,
        transaction_context: &TransactionContext,
    ) -> Vec<SQLOperation<'a>> {
        match self {
            TemplateSQLOperation::Insert(insert) => insert
                .resolve(prev_step_id, transaction_context)
                .into_iter()
                .map(SQLOperation::Insert)
                .collect(),
            TemplateSQLOperation::Update(update) => update
                .resolve(prev_step_id, transaction_context)
                .into_iter()
                .map(SQLOperation::Update)
                .collect(),
            TemplateSQLOperation::Delete(delete) => {
                vec![SQLOperation::Delete(delete.resolve())]
            }
        }
    }
}
