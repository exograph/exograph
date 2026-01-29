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
    pub doc_comments: Option<String>,
}

/// A simple scalar parameter (e.g., limit, offset).
#[derive(Serialize, Deserialize, Debug)]
pub struct ScalarParam {
    pub name: String,
    pub description: String,
    pub type_name: String,
}

/// Enum for iterating over collection query parameters generically.
pub enum CollectionQueryParam<'a> {
    Predicate(&'a PredicateParameter),
    OrderBy(&'a OrderByParameter),
    Scalar(&'a ScalarParam),
}

/// Parameters for collection queries (e.g., `get_todos`)
#[derive(Serialize, Deserialize, Debug)]
pub struct CollectionQueryParameters {
    pub predicate_param: PredicateParameter,
    pub order_by_param: OrderByParameter,
    pub limit_param: ScalarParam,
    pub offset_param: ScalarParam,
}

impl CollectionQueryParameters {
    /// Return all parameters for generic iteration (e.g., schema building).
    pub fn params(&self) -> Vec<CollectionQueryParam<'_>> {
        vec![
            CollectionQueryParam::Predicate(&self.predicate_param),
            CollectionQueryParam::OrderBy(&self.order_by_param),
            CollectionQueryParam::Scalar(&self.limit_param),
            CollectionQueryParam::Scalar(&self.offset_param),
        ]
    }
}

/// Parameters for pk queries (e.g., `get_todo`)
#[derive(Serialize, Deserialize, Debug)]
pub struct PkQueryParameters {
    /// Predicate parameters for each pk field (implicit equality)
    pub predicate_params: Vec<PredicateParameter>,
}

pub type CollectionQuery = PostgresOperation<CollectionQueryParameters>;
pub type PkQuery = PostgresOperation<PkQueryParameters>;
