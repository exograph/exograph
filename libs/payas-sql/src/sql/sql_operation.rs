use super::{
    cte::Cte,
    delete::Delete,
    delete::TemplateDelete,
    insert::{Insert, TemplateInsert},
    select::Select,
    transaction::{TransactionContext, TransactionStepId},
    update::{TemplateUpdate, Update},
    Expression, SQLBuilder,
};

#[derive(Debug)]
pub enum SQLOperation<'a> {
    Select(Select<'a>),
    Insert(Insert<'a>),
    Delete(Delete<'a>),
    Update(Update<'a>),
    Cte(Cte<'a>),
}

impl<'a> Expression for SQLOperation<'a> {
    fn binding(&self, builder: &mut SQLBuilder) {
        match self {
            SQLOperation::Select(select) => select.binding(builder),
            SQLOperation::Insert(insert) => insert.binding(builder),
            SQLOperation::Delete(delete) => delete.binding(builder),
            SQLOperation::Update(update) => update.binding(builder),
            SQLOperation::Cte(cte) => cte.binding(builder),
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
