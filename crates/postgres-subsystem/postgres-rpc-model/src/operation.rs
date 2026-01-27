// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use core_model::types::OperationReturnType;
use postgres_core_model::order::OrderByParameter;
use postgres_core_model::predicate::PredicateParameter;
use postgres_core_model::types::EntityType;
use serde::{Deserialize, Serialize};

// TODO: Share this with REST?

/// Base operation type for RPC operations
#[derive(Serialize, Deserialize, Debug)]
pub struct PostgresOperation<P> {
    pub name: String,
    pub parameters: P,
    pub return_type: OperationReturnType<EntityType>,
}

/// Parameters for collection queries (e.g., `get_todos`)
#[derive(Serialize, Deserialize, Debug)]
pub struct CollectionQueryParameters {
    pub predicate_param: PredicateParameter,
    pub order_by_param: OrderByParameter,
}

/// Parameters for pk queries (e.g., `get_todo`)
#[derive(Serialize, Deserialize, Debug)]
pub struct PkQueryParameters {
    /// Predicate parameters for each pk field (implicit equality)
    pub predicate_params: Vec<PredicateParameter>,
}

pub type CollectionQuery = PostgresOperation<CollectionQueryParameters>;
pub type PkQuery = PostgresOperation<PkQueryParameters>;
