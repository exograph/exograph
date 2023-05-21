// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use crate::{
    asql::{
        abstract_operation::AbstractOperation, delete::AbstractDelete, insert::AbstractInsert,
        select::AbstractSelect, update::AbstractUpdate,
    },
    sql::{
        cte::WithQuery, order::OrderBy, predicate::ConcretePredicate, select::Select,
        transaction::TransactionScript,
    },
    AbstractOrderBy, AbstractPredicate, Database,
};

use super::pg::Postgres;

/// Transform an abstract operation into a transaction script
pub trait OperationTransformer {
    fn to_transaction_script<'a>(
        &self,
        database: &'a Database,
        abstract_operation: &'a AbstractOperation,
    ) -> TransactionScript<'a>;
}

impl OperationTransformer for Postgres {
    fn to_transaction_script<'a>(
        &self,
        database: &'a Database,
        abstract_operation: &'a AbstractOperation,
    ) -> TransactionScript<'a> {
        match abstract_operation {
            AbstractOperation::Select(select) => {
                SelectTransformer::to_transaction_script(self, select, database)
            }
            AbstractOperation::Delete(delete) => {
                DeleteTransformer::to_transaction_script(self, delete, database)
            }
            AbstractOperation::Insert(insert) => {
                InsertTransformer::to_transaction_script(self, insert, database)
            }
            AbstractOperation::Update(update) => {
                UpdateTransformer::to_transaction_script(self, update, database)
            }
        }
    }
}

pub trait SelectTransformer {
    fn to_select<'a>(
        &self,
        abstract_select: &AbstractSelect<'a>,
        database: &'a Database,
    ) -> Select<'a>;

    fn to_transaction_script<'a>(
        &self,
        abstract_select: &'a AbstractSelect,
        database: &'a Database,
    ) -> TransactionScript<'a>;
}

pub trait DeleteTransformer {
    fn to_delete<'a>(
        &self,
        abstract_delete: &'a AbstractDelete,
        database: &'a Database,
    ) -> WithQuery<'a>;

    fn to_transaction_script<'a>(
        &self,
        abstract_delete: &'a AbstractDelete,
        database: &'a Database,
    ) -> TransactionScript<'a>;
}

pub trait InsertTransformer {
    fn to_transaction_script<'a>(
        &self,
        abstract_insert: &'a AbstractInsert,
        database: &'a Database,
    ) -> TransactionScript<'a>;
}

pub trait UpdateTransformer {
    fn to_transaction_script<'a>(
        &self,
        abstract_update: &'a AbstractUpdate,
        database: &'a Database,
    ) -> TransactionScript<'a>;
}

pub trait PredicateTransformer {
    fn to_predicate<'a>(
        &self,
        predicate: &AbstractPredicate,
        assume_tables_in_context: bool,
        database: &'a Database,
    ) -> ConcretePredicate<'a>;
}

pub trait OrderByTransformer {
    fn to_order_by(&self, order_by: &AbstractOrderBy) -> OrderBy;
}
