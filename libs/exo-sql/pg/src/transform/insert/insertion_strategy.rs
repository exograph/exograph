// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use crate::PgAbstractInsert;
use crate::transaction::{TransactionScript, TransactionStepId};
use exo_sql_core::{ColumnId, Database};

use crate::pg::Postgres;

/// A strategy for generating a SQL query from an abstract insert.
pub(crate) trait InsertionStrategy {
    /// A unique identifier for this strategy (for debugging purposes)
    fn id(&self) -> &'static str;

    /// See `SelectionStrategy::suitable`
    fn suitable(&self, abstract_insert: &PgAbstractInsert, database: &Database) -> bool;

    fn update_transaction_script<'a>(
        &self,
        abstract_insert: PgAbstractInsert,
        parent_step: Option<(TransactionStepId, Vec<ColumnId>)>,
        database: &'a Database,
        transformer: &Postgres,
        transaction_script: &mut TransactionScript<'a>,
    );
}
