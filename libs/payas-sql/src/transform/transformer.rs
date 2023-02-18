use crate::{
    asql::{
        abstract_operation::AbstractOperation,
        delete::AbstractDelete,
        insert::AbstractInsert,
        select::{AbstractSelect, SelectionLevel},
        update::AbstractUpdate,
    },
    sql::{select::Select, transaction::TransactionScript},
    Predicate,
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
                SelectTransformer::to_transaction_script(self, select)
            }
            AbstractOperation::Delete(delete) => {
                DeleteTransformer::to_transaction_script(self, delete)
            }
            AbstractOperation::Insert(insert) => {
                InsertTransformer::to_transaction_script(self, insert)
            }
            AbstractOperation::Update(update) => {
                UpdateTransformer::to_transaction_script(self, update)
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
    ) -> TransactionScript<'a>;
}

pub trait DeleteTransformer {
    fn to_transaction_script<'a>(
        &self,
        abstract_delete: &'a AbstractDelete,
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
    ) -> TransactionScript<'a>;
}
