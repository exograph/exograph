// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use crate::{
    Database,
    database_error::DatabaseError,
    sql::{
        connect::database_client_manager::DatabaseClientManager, transaction::TransactionStepResult,
    },
    transform::{pg::Postgres, transformer::OperationTransformer},
};

use super::{abstract_operation::AbstractOperation, transaction_holder::TransactionHolder};

pub struct DatabaseExecutor {
    pub database_client: DatabaseClientManager,
}

impl DatabaseExecutor {
    /// Execute an operation on a database.
    ///
    /// Currently makes a hard assumption on Postgres implementation, but this could be made more generic.
    pub async fn execute(
        &self,
        operation: AbstractOperation,
        tx_holder: &mut TransactionHolder,
        database: &Database,
    ) -> Result<TransactionStepResult, DatabaseError> {
        let database_kind = Postgres {};
        let transaction_script = database_kind.to_transaction_script(database, operation);

        tx_holder
            .with_tx(database, &self.database_client, transaction_script)
            .await
    }
}
