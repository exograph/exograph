// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use async_trait::async_trait;
use exo_sql_core::{Database, DatabaseError};
use exo_sql_model::{AbstractOperation, DatabaseBackend};
use exo_sql_pg::{PgExtension, pg::Postgres};

use crate::{TransactionHolder, connect::database_client_manager::DatabaseClientManager};

/// Postgres implementation of `DatabaseBackend`.
///
/// Transforms abstract operations into Postgres transaction scripts,
/// executes them, and converts the result rows to strings.
pub struct PgBackend {
    database_client: DatabaseClientManager,
}

impl PgBackend {
    pub fn new(database_client: DatabaseClientManager) -> Self {
        Self { database_client }
    }
}

#[async_trait]
impl DatabaseBackend for PgBackend {
    type Ext = PgExtension;
    type TxHolder = TransactionHolder;

    async fn execute(
        &self,
        operation: AbstractOperation<PgExtension>,
        tx_holder: &mut TransactionHolder,
        database: &Database,
    ) -> Result<Vec<String>, DatabaseError> {
        let pg = Postgres {};
        let transaction_script = pg.to_transaction_script(database, operation);

        let rows = tx_holder
            .with_tx(database, &self.database_client, transaction_script)
            .await?;

        rows.into_iter()
            .map(|row| row.try_get::<_, String>(0).map_err(DatabaseError::driver))
            .collect()
    }
}
