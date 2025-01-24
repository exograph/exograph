// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

//! Transform an abstract update into a transaction script.
//!
//! This allows us to execute GraphQL mutations like this:
//!
//! ```graphql
//! mutation {
//!   updateConcert(id: 4, data: {
//!     title: "new-title",
//!     concertArtists: {
//!       create: [{artist: {id: 30}, rank: 2, role: "main"}],
//!       update: [{id: 100, artist: {id: 10}, rank: 2}, {id: 101, artist: {id: 10}, role: "accompanying"}],
//!       update: [{id: 110}]
//!     }
//!   }) {
//!     id
//!   }
//! }
//! ```
//!
use tracing::instrument;

use crate::{
    sql::transaction::TransactionScript,
    transform::{pg::Postgres, transformer::UpdateTransformer},
    AbstractUpdate, Database,
};

use super::update_strategy_chain::UpdateStrategyChain;

impl UpdateTransformer for Postgres {
    #[instrument(
        name = "UpdateTransformer::to_transaction_script for Postgres"
        skip(self, database, transaction_script)
        )]
    fn update_transaction_script<'a>(
        &self,
        abstract_update: AbstractUpdate,
        database: &'a Database,
        transaction_script: &mut TransactionScript<'a>,
    ) {
        let chain = UpdateStrategyChain::default();

        chain.update_transaction_script(abstract_update, database, self, transaction_script);
    }
}
