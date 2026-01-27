// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use postgres_core_model::order::OrderByParameter;
use postgres_core_model::predicate::PredicateParameter;
use serde::{Deserialize, Serialize};

use core_model::type_normalization::Parameter;

use crate::limit_offset::{LimitParameter, OffsetParameter};

use super::operation::{OperationParameters, PostgresOperation};

/// Query that return a collection such as `todos(where: { title: { eq: "Hello" } })`
pub type CollectionQuery = PostgresOperation<CollectionQueryParameters>;

/// Collection query parameters
#[derive(Serialize, Deserialize, Debug)]
pub struct CollectionQueryParameters {
    /// The predicate parameter such as `where: { title: { eq: "Hello" } }`
    pub predicate_param: PredicateParameter,
    /// The order by parameter such as `orderBy: { title: ASC }`
    pub order_by_param: OrderByParameter,
    /// The limit parameter such as `limit: 10`
    pub limit_param: LimitParameter,
    /// The offset parameter such as `offset: 20`
    pub offset_param: OffsetParameter,
}

impl OperationParameters for CollectionQueryParameters {
    fn introspect(&self) -> Vec<&dyn Parameter> {
        vec![
            &self.predicate_param,
            &self.order_by_param,
            &self.limit_param,
            &self.offset_param,
        ]
    }
}

/// Query that returns an aggregate such as `todosAgg(where: { title: { eq: "Hello" } })`
pub type AggregateQuery = PostgresOperation<AggregateQueryParameters>;

/// Query parameter such as `id: 1` in `todo(id: 1)` to be used in an aggregate query
#[derive(Serialize, Deserialize, Debug)]
pub struct AggregateQueryParameters {
    pub predicate_param: PredicateParameter,
}

impl OperationParameters for AggregateQueryParameters {
    fn introspect(&self) -> Vec<&dyn Parameter> {
        vec![&self.predicate_param]
    }
}

/// Query that returns a single entity (due to the constraint)
/// - Primary key: such as single `todo(id: 1)` or composite `user(firstName: "John", lastName: "Doe")`
/// - Uniqueness: such as `userByEmail(email: "hello@example.com")` or `userByFirstAndLastName(firstName: "John", lastName: "Doe")`
pub type UniqueQuery = PostgresOperation<UniqueQueryParameters>;

#[derive(Serialize, Deserialize, Debug)]
pub struct UniqueQueryParameters {
    pub predicate_params: Vec<PredicateParameter>,
}

impl OperationParameters for UniqueQueryParameters {
    fn introspect(&self) -> Vec<&dyn Parameter> {
        self.predicate_params
            .iter()
            .map(|p| p as &dyn Parameter)
            .collect()
    }
}
