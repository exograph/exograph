// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use exo_sql_core::Database;
use exo_sql_pg_core::PgAbstractDelete;
use exo_sql_pg_core::transaction::TransactionScript;

use crate::pg::Postgres;
use crate::pg::pg_transformer::PgDeleteTransformer;

use super::delete_strategy_chain::DeleteStrategyChain;

impl PgDeleteTransformer for Postgres {
    fn update_transaction_script<'a>(
        &self,
        abstract_delete: PgAbstractDelete,
        database: &'a Database,
        transaction_script: &mut TransactionScript<'a>,
    ) {
        let chain = DeleteStrategyChain::default();

        chain.update_transaction_script(abstract_delete, database, self, transaction_script);
    }
}
