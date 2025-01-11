// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

//! Transform an abstract insert into a concrete insert for Postgres.
//!
//! This allows us to execute GraphQL mutations like this:
//!
//! ```graphql
//! mutation {
//!   createVenue(data: {name: "v1", published: true, latitude: 1.2, concerts: [
//!     {title: "c1", published: true, price: 1.2},
//!     {title: "c2", published: false, price: 2.4}
//!   ]}) {
//!     id
//!   }
//! }
//! ```

use super::insertion_strategy_chain::InsertionStrategyChain;
use crate::{
    sql::transaction::{TransactionScript, TransactionStepId},
    transform::{pg::Postgres, transformer::InsertTransformer},
    AbstractInsert, ColumnId, Database,
};

impl InsertTransformer for Postgres {
    fn update_transaction_script<'a>(
        &self,
        abstract_insert: &'a AbstractInsert,
        parent_step: Option<(TransactionStepId, Vec<ColumnId>)>,
        database: &'a Database,
        transaction_script: &mut TransactionScript<'a>,
    ) {
        let chain = InsertionStrategyChain::default();

        chain.update_transaction_script(
            abstract_insert,
            parent_step,
            database,
            self,
            transaction_script,
        );
    }
}
