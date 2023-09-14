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
    sql::transaction::TransactionScript, transform::pg::Postgres, AbstractDelete, Database,
};

use super::{cte_strategy::CteStrategy, deletion_strategy::DeletionStrategy};

/// Chain of various deletion strategies.
pub(crate) struct DeletionStrategyChain<'s> {
    strategies: Vec<&'s dyn DeletionStrategy>,
}

impl<'s> DeletionStrategyChain<'s> {
    /// Create a new deletion strategy chain.
    pub fn new(strategies: Vec<&'s dyn DeletionStrategy>) -> Self {
        Self { strategies }
    }

    /// Find the first strategy that is suitable for the given deletion, and update the
    /// `TransactionScript` with steps to execute.
    pub fn update_transaction_script<'a>(
        &self,
        abstract_delete: &'a AbstractDelete,
        database: &'a Database,
        transformer: &Postgres,
        transaction_script: &mut TransactionScript<'a>,
    ) {
        let strategy = self
            .strategies
            .iter()
            .find(|s| s.suitable(abstract_delete, database))
            .unwrap();

        debug!("Using deletion strategy: {}", strategy.id());

        strategy.update_transaction_script(
            abstract_delete,
            database,
            transformer,
            transaction_script,
        );
    }
}

impl Default for DeletionStrategyChain<'_> {
    fn default() -> Self {
        Self::new(vec![&CteStrategy {}])
    }
}
