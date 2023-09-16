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
    sql::transaction::TransactionScript, transform::pg::Postgres, AbstractUpdate, Database,
};

use super::{
    cte_strategy::CteStrategy, multi_statement_strategy::MultiStatementStrategy,
    update_strategy::UpdateStrategy,
};

/// Chain of various deletion strategies.
pub(crate) struct UpdateStrategyChain<'s> {
    strategies: Vec<&'s dyn UpdateStrategy>,
}

impl<'s> UpdateStrategyChain<'s> {
    /// Create a new update strategy chain.
    pub fn new(strategies: Vec<&'s dyn UpdateStrategy>) -> Self {
        Self { strategies }
    }

    /// Find the first strategy that is suitable for the given update, and update the
    /// `TransactionScript` with steps to execute.
    pub fn update_transaction_script<'a>(
        &self,
        abstract_update: &'a AbstractUpdate,
        database: &'a Database,
        transformer: &Postgres,
        transaction_script: &mut TransactionScript<'a>,
    ) {
        let strategy = self
            .strategies
            .iter()
            .find(|s| s.suitable(abstract_update, database))
            .unwrap();

        debug!("Using update strategy: {}", strategy.id());

        strategy.update_transaction_script(
            abstract_update,
            database,
            transformer,
            transaction_script,
        );
    }
}

impl Default for UpdateStrategyChain<'_> {
    fn default() -> Self {
        Self::new(vec![&CteStrategy {}, &MultiStatementStrategy {}])
    }
}
