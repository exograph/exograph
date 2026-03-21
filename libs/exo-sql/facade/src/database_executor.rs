// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use exo_sql_core::{Database, DatabaseError};
use exo_sql_model::AbstractOperation;
use exo_sql_pg_connect::{
    TransactionHolder, connect::database_client_manager::DatabaseClientManager,
};
use exo_sql_pg_core::{PgExtension, TransactionStepResult};
use exo_sql_pg_transform::pg::Postgres;

pub struct DatabaseExecutor {
    pub database_client: DatabaseClientManager,
}

impl DatabaseExecutor {
    /// Execute an operation on a database.
    ///
    /// Transforms the abstract operation into a Postgres transaction script,
    /// then executes it.
    pub async fn execute(
        &self,
        operation: AbstractOperation<PgExtension>,
        tx_holder: &mut TransactionHolder,
        database: &Database,
    ) -> Result<TransactionStepResult, DatabaseError> {
        let pg = Postgres {};
        let transaction_script = pg.to_transaction_script(database, operation);

        tx_holder
            .with_tx(database, &self.database_client, transaction_script)
            .await
    }
}
