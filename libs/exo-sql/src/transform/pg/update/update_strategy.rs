// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use crate::{
    sql::transaction::TransactionScript, transform::pg::Postgres, AbstractUpdate, Database,
};

/// A strategy for generating a transaction script from an abstract update.
pub(crate) trait UpdateStrategy {
    /// A unique identifier for this strategy (for debugging purposes)
    fn id(&self) -> &'static str;

    /// See `SelectionStrategy::suitable`
    fn suitable(&self, abstract_update: &AbstractUpdate, database: &Database) -> bool;

    fn update_transaction_script<'a>(
        &self,
        abstract_update: &'a AbstractUpdate,
        database: &'a Database,
        transformer: &Postgres,
        transaction_script: &mut TransactionScript<'a>,
    );
}
