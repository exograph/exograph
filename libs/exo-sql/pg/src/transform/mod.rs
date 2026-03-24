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

pub(crate) mod join_util;
pub(crate) mod precheck;
pub(crate) mod table_dependency;
#[cfg(test)]
pub(crate) mod transform_test_util;

mod order_by_transformer;
pub(crate) mod pg_transformer;
mod predicate_transformer;

pub struct Postgres {}

use crate::{PgAbstractOperation, TransactionScript};
use exo_sql_core::Database;
use exo_sql_model::AbstractOperation;
use pg_transformer::{
    PgDeleteTransformer, PgInsertTransformer, PgSelectTransformer, PgUpdateTransformer,
};

impl Postgres {
    pub fn to_transaction_script<'a>(
        &self,
        database: &'a Database,
        abstract_operation: PgAbstractOperation,
    ) -> TransactionScript<'a> {
        match abstract_operation {
            AbstractOperation::Select(select) => {
                PgSelectTransformer::to_transaction_script(self, select, database)
            }
            AbstractOperation::Delete(delete) => {
                PgDeleteTransformer::to_transaction_script(self, delete, database)
            }
            AbstractOperation::Insert(insert) => {
                PgInsertTransformer::to_transaction_script(self, insert, None, database)
            }
            AbstractOperation::Update(update) => {
                PgUpdateTransformer::to_transaction_script(self, update, database)
            }
        }
    }
}
