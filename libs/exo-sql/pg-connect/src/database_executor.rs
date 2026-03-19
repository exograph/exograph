// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use exo_sql_core::{Database, database_error::DatabaseError};
use exo_sql_pg_core::transaction::{TransactionScript, TransactionStepResult};

use crate::{
    connect::database_client_manager::DatabaseClientManager, transaction_holder::TransactionHolder,
};

pub struct DatabaseExecutor {
    pub database_client: DatabaseClientManager,
}

impl DatabaseExecutor {
    /// Execute a transaction script on a database.
    pub async fn execute(
        &self,
        transaction_script: TransactionScript<'_>,
        tx_holder: &mut TransactionHolder,
        database: &Database,
    ) -> Result<TransactionStepResult, DatabaseError> {
        tx_holder
            .with_tx(database, &self.database_client, transaction_script)
            .await
    }
}
