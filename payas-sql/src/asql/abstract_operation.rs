use crate::sql::transaction::TransactionScript;

use super::{
    delete::AbstractDelete, insert::AbstractInsert, select::AbstractSelect, update::AbstractUpdate,
};

pub enum AbstractOperation<'a> {
    Select(AbstractSelect<'a>),
    Delete(AbstractDelete<'a>),
    Insert(AbstractInsert<'a>),
    Update(AbstractUpdate<'a>),
}

impl<'a> AbstractOperation<'a> {
    pub(crate) fn to_transaction_script(&'a self) -> TransactionScript<'a> {
        match self {
            AbstractOperation::Select(select) => select.to_transaction_script(None),
            AbstractOperation::Delete(delete) => delete.to_transaction_script(None),
            AbstractOperation::Insert(insert) => insert.to_transaction_script(),
            AbstractOperation::Update(update) => update.to_transaction_script(None),
        }
    }
}
