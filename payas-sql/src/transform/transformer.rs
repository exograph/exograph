use crate::{
    asql::{
        abstract_operation::AbstractOperation,
        delete::AbstractDelete,
        insert::AbstractInsert,
        select::{AbstractSelect, SelectionLevel},
        update::AbstractUpdate,
    },
    sql::{predicate::Predicate, select::Select, transaction::TransactionScript},
};

use super::pg::Postgres;

pub trait Transformer {
    fn to_transaction_script<'a>(
        &self,
        abstract_operation: &'a AbstractOperation,
    ) -> TransactionScript<'a>;
}

impl Transformer for Postgres {
    fn to_transaction_script<'a>(
        &self,
        abstract_operation: &'a AbstractOperation,
    ) -> TransactionScript<'a> {
        match abstract_operation {
            AbstractOperation::Select(select) => {
                SelectTransformer::to_transaction_script(self, select, None)
            }
            AbstractOperation::Delete(delete) => {
                DeleteTransformer::to_transaction_script(self, delete, None)
            }
            AbstractOperation::Insert(insert) => {
                InsertTransformer::to_transaction_script(self, insert)
            }
            AbstractOperation::Update(update) => {
                UpdateTransformer::to_transaction_script(self, update, None)
            }
        }
    }
}

pub trait SelectTransformer {
    fn to_select<'a>(
        &self,
        abstract_select: &'a AbstractSelect,
        additional_predicate: Option<Predicate<'a>>,
        selection_level: SelectionLevel,
    ) -> Select<'a>;

    fn to_transaction_script<'a>(
        &self,
        abstract_select: &'a AbstractSelect,
        additional_predicate: Option<Predicate<'a>>,
    ) -> TransactionScript<'a>;
}

// impl Transformer for Postgres {
//     fn transform(&self, abstract_operation: &AbstractOperation) -> TransactionScript {
//         match abstract_operation {
//             AbstractOperation::Select(select) => select.to_transaction_script(None),
//             AbstractOperation::Delete(delete) => delete.to_transaction_script(None),
//             AbstractOperation::Insert(insert) => insert.to_transaction_script(),
//             AbstractOperation::Update(update) => update.to_transaction_script(None),
//         }
//     }
// }

pub trait DeleteTransformer {
    fn to_transaction_script<'a>(
        &self,
        abstract_delete: &'a AbstractDelete,
        additional_predicate: Option<Predicate<'a>>,
    ) -> TransactionScript<'a>;
}

pub trait InsertTransformer {
    fn to_transaction_script<'a>(
        &self,
        abstract_insert: &'a AbstractInsert,
    ) -> TransactionScript<'a>;
}

pub trait UpdateTransformer {
    fn to_transaction_script<'a>(
        &self,
        abstract_update: &'a AbstractUpdate,
        additional_predicate: Option<Predicate<'a>>,
    ) -> TransactionScript<'a>;
}
