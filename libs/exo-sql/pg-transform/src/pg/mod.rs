// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

mod delete;
mod insert;
mod select;
mod update;

pub(crate) mod precheck;

mod order_by_transformer;
mod predicate_transformer;

pub struct Postgres {}

use exo_sql_core::Database;
use exo_sql_model::abstract_operation::AbstractOperation;
use exo_sql_model::transformer::{
    DeleteTransformer, InsertTransformer, SelectTransformer, UpdateTransformer,
};
use exo_sql_pg_core::TransactionScript;

impl Postgres {
    pub fn to_transaction_script<'a>(
        &self,
        database: &'a Database,
        abstract_operation: AbstractOperation,
    ) -> TransactionScript<'a> {
        match abstract_operation {
            AbstractOperation::Select(select) => {
                SelectTransformer::to_transaction_script(self, select, database)
            }
            AbstractOperation::Delete(delete) => {
                DeleteTransformer::to_transaction_script(self, delete, database)
            }
            AbstractOperation::Insert(insert) => {
                InsertTransformer::to_transaction_script(self, insert, None, database)
            }
            AbstractOperation::Update(update) => {
                UpdateTransformer::to_transaction_script(self, update, database)
            }
        }
    }
}
