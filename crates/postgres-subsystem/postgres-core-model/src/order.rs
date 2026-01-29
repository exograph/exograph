// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use core_model::{
    mapped_arena::SerializableSlabIndex,
    type_normalization::{Parameter, Type},
    types::{FieldType, Named, TypeValidation},
};

use crate::access::Access;

use exo_sql::{ColumnPathLink, VectorDistanceFunction};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct OrderByParameter {
    /// The name of the parameter. For example, "orderBy", "title", "venue", etc.
    pub name: String,

    /// For composite parameters, FieldType will be a list to maintain ordering.
    pub typ: FieldType<OrderByParameterTypeWrapper>,

    /// How does this parameter relate to the parent parameter?
    /// For example for parameter used as `{order_by: {venue1: {id: Desc}}}`, we will have following column links:
    /// ```no_rust
    ///   id: Some((<the venues.id column>, None))
    ///   venue1: Some((<the concerts.venue1_id column>, <the venues.id column>))
    ///   order_by: None
    /// ```
    pub column_path_link: Option<ColumnPathLink>,
    pub access: Option<Access>,
    // TODO: Generalize this to support more than just vector distance functions
    pub vector_distance_function: Option<VectorDistanceFunction>,
}

/// Wrapper around OrderByParameterType to satisfy the Named trait without cloning the parameter type.
/// This provides a name for the parameter type while holding a pointer to the actual parameter type.
#[derive(Serialize, Deserialize, Debug)]
pub struct OrderByParameterTypeWrapper {
    pub name: String,
    /// Type id of the parameter type. For example: Ordering, ConcertOrdering, etc.
    pub type_id: SerializableSlabIndex<OrderByParameterType>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct OrderByParameterType {
    /// The name of the type. For example, "Ordering", "ConcertOrdering".
    pub name: String,
    pub kind: OrderByParameterTypeKind,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum OrderByParameterTypeKind {
    Primitive,
    Vector,
    Composite { parameters: Vec<OrderByParameter> },
}

pub const ORDER_BY_PARAM_NAME: &str = "orderBy";
pub const PRIMITIVE_ORDERING_TYPE_NAME: &str = "Ordering";
pub const PRIMITIVE_ORDERING_OPTIONS: [&str; 2] = ["ASC", "DESC"];

impl Named for OrderByParameterTypeWrapper {
    fn name(&self) -> &str {
        &self.name
    }
}

impl Parameter for OrderByParameter {
    fn name(&self) -> &str {
        &self.name
    }

    fn typ(&self) -> Type {
        (&self.typ).into()
    }

    fn type_validation(&self) -> Option<TypeValidation> {
        None
    }
}
