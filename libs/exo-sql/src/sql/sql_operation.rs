// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use crate::Database;

use super::{
    cte::WithQuery,
    delete::Delete,
    delete::TemplateDelete,
    insert::{Insert, TemplateInsert},
    select::Select,
    transaction::{TransactionContext, TransactionStepId},
    update::{TemplateUpdate, Update},
    ExpressionBuilder, SQLBuilder,
};

/// Top-level SQL operation, which may be executed by the database.
#[derive(Debug)]
pub enum SQLOperation<'a> {
    Select(Select<'a>),
    Insert(Insert<'a>),
    Delete(Delete<'a>),
    Update(Update<'a>),
    WithQuery(WithQuery<'a>),
}

impl<'a> ExpressionBuilder for SQLOperation<'a> {
    fn build(&self, database: &Database, builder: &mut SQLBuilder) {
        match self {
            SQLOperation::Select(select) => select.build(database, builder),
            SQLOperation::Insert(insert) => insert.build(database, builder),
            SQLOperation::Delete(delete) => delete.build(database, builder),
            SQLOperation::Update(update) => update.build(database, builder),
            SQLOperation::WithQuery(cte) => cte.build(database, builder),
        }
    }
}

/// A SQL operation that may contain template columns (whose value isn't know until an earlier step is executed)
#[derive(Debug)]
pub enum TemplateSQLOperation<'a> {
    Insert(TemplateInsert<'a>),
    Update(TemplateUpdate<'a>),
    Delete(TemplateDelete<'a>),
}

impl<'a> TemplateSQLOperation<'a> {
    // Resolve the template operation into a concrete operation.
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
