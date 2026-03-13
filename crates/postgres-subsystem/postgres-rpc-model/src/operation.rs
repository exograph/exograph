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

/// Trait for parameter types that have a list of predicate params (used for PK/unique lookup matching).
pub trait HasPredicateParams {
    fn predicate_params(&self) -> &[PredicateParameter];
}

/// Define a predicate-list parameter struct with HasPredicateParams and From impls,
/// plus a type alias for PostgresOperation<...>.
macro_rules! define_predicate_params {
    ($($(#[doc = $doc:expr])* $params_name:ident => $op_alias:ident),+ $(,)?) => { $(
        $(#[doc = $doc])*
        #[derive(Serialize, Deserialize, Debug)]
        pub struct $params_name {
            pub predicate_params: Vec<PredicateParameter>,
        }

        impl HasPredicateParams for $params_name {
            fn predicate_params(&self) -> &[PredicateParameter] {
                &self.predicate_params
            }
        }

        impl From<Vec<PredicateParameter>> for $params_name {
            fn from(predicate_params: Vec<PredicateParameter>) -> Self {
                Self { predicate_params }
            }
        }

        pub type $op_alias = PostgresOperation<$params_name>;
    )+ };
}

define_predicate_params!(
    /// Parameters for pk queries (e.g., `get_todo`)
    PkQueryParameters => PkQuery,
    /// Parameters for unique constraint queries (e.g., `get_user` by username)
    UniqueQueryParameters => UniqueQuery,
    /// Parameters for single delete by PK (e.g., `delete_todo`)
    PkDeleteParameters => PkDelete,
    /// Parameters for single delete by unique constraint
    UniqueDeleteParameters => UniqueDelete,
);

/// A parameter representing the data payload for create/update operations.
#[derive(Serialize, Deserialize, Debug)]
pub struct DataParam {
    pub name: String,
}

impl DataParam {
    pub const DEFAULT_NAME: &str = "data";
}

impl Default for DataParam {
    fn default() -> Self {
        Self {
            name: Self::DEFAULT_NAME.to_string(),
        }
    }
}

/// Define a predicate+data parameter struct with HasPredicateParams impl,
/// plus a type alias for PostgresOperation<...>.
macro_rules! define_predicate_data_params {
    ($($(#[doc = $doc:expr])* $params_name:ident => $op_alias:ident),+ $(,)?) => { $(
        $(#[doc = $doc])*
        #[derive(Serialize, Deserialize, Debug)]
        pub struct $params_name {
            pub predicate_params: Vec<PredicateParameter>,
            pub data_param: DataParam,
        }

        impl HasPredicateParams for $params_name {
            fn predicate_params(&self) -> &[PredicateParameter] {
                &self.predicate_params
            }
        }

        pub type $op_alias = PostgresOperation<$params_name>;
    )+ };
}

define_predicate_data_params!(
    /// Parameters for single update by PK (e.g., `update_todo`)
    PkUpdateParameters => PkUpdate,
    /// Parameters for single update by unique constraint
    UniqueUpdateParameters => UniqueUpdate,
);

/// Define a collection parameter struct (single predicate_param) plus a type alias.
macro_rules! define_collection_params {
    ($($(#[doc = $doc:expr])* $params_name:ident => $op_alias:ident),+ $(,)?) => { $(
        $(#[doc = $doc])*
        #[derive(Serialize, Deserialize, Debug)]
        pub struct $params_name {
            pub predicate_param: PredicateParameter,
        }

        pub type $op_alias = PostgresOperation<$params_name>;
    )+ };
}

pub type CollectionQuery = PostgresOperation<CollectionQueryParameters>;

define_collection_params!(
    /// Parameters for collection delete (e.g., `delete_todos`)
    CollectionDeleteParameters => CollectionDelete,
);

/// Parameters for collection update (e.g., `update_todos`)
#[derive(Serialize, Deserialize, Debug)]
pub struct CollectionUpdateParameters {
    pub predicate_param: PredicateParameter,
    pub data_param: DataParam,
}

pub type CollectionUpdate = PostgresOperation<CollectionUpdateParameters>;

/// Parameters for create operations (single or collection)
#[derive(Serialize, Deserialize, Debug)]
pub struct CreateParameters {
    pub data_param: DataParam,
}

pub type Create = PostgresOperation<CreateParameters>;
pub type CollectionCreate = PostgresOperation<CreateParameters>;

/// Delegate to the inner parameters type.
impl<P: HasPredicateParams> HasPredicateParams for PostgresOperation<P> {
    fn predicate_params(&self) -> &[PredicateParameter] {
        self.parameters.predicate_params()
    }
}
