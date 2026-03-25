// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use async_trait::async_trait;
use exo_sql_core::operation::DatabaseExtension;
use exo_sql_core::{Database, DatabaseError};

use crate::operation::AbstractOperation;

/// Trait for database backends that can execute abstract operations.
///
/// Generic over the database extension type (`Ext`) and transaction holder (`TxHolder`),
/// so each backend defines its own operation and transaction types.
/// Currently the only implementation is `PgBackend` (Postgres) in the `pg-connect` crate.
#[async_trait]
pub trait DatabaseBackend: Send + Sync {
    type Ext: DatabaseExtension;
    type TxHolder: Send;

    async fn execute(
        &self,
        operation: AbstractOperation<Self::Ext>,
        tx_holder: &mut Self::TxHolder,
        database: &Database,
    ) -> Result<Vec<String>, DatabaseError>;
}
