// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use tracing::debug;

use crate::{
    sql::transaction::TransactionScript, transform::pg::Postgres, AbstractInsert, Database,
};

use super::{
    insertion_strategy::InsertionStrategy, multi_statement_strategy::MultiStatementStrategy,
};

/// Chain of various insertion strategies.
/// Currently, we have only one strategy, but we may add more in the future.
pub(crate) struct InsertionStrategyChain<'s> {
    strategies: Vec<&'s dyn InsertionStrategy>,
}

impl<'s> InsertionStrategyChain<'s> {
    /// Create a new Insertion strategy chain.
    pub fn new(strategies: Vec<&'s dyn InsertionStrategy>) -> Self {
        Self { strategies }
    }

    /// Find the first strategy that is suitable for the given insertion context, and return a
    /// `TransactionScript` to execute.
    pub fn to_transaction_script<'a>(
        &self,
        abstract_insert: &'a AbstractInsert,
        database: &'a Database,
        transformer: &Postgres,
    ) -> Option<TransactionScript<'a>> {
        let strategy = self
            .strategies
            .iter()
            .find(|s| s.suitable(abstract_insert, database))?;

        debug!("Using insertion strategy: {}", strategy.id());

        Some(strategy.to_transaction_script(abstract_insert, database, transformer))
    }
}

impl Default for InsertionStrategyChain<'_> {
    fn default() -> Self {
        Self::new(vec![&MultiStatementStrategy {}])
    }
}
