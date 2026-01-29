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

use crate::{access::Access, types::EntityType};

use exo_sql::{ColumnPathLink, VectorDistanceFunction};
use serde::{Deserialize, Serialize};

pub const PREDICATE_PARAM_NAME: &str = "where";

#[derive(Serialize, Deserialize, Debug)]
pub struct PredicateParameter {
    /// The name of the parameter. For example, "where", "and", "id", "venue", etc.
    pub name: String,

    /// For parameters such as "and", FieldType will be a list.
    pub typ: FieldType<PredicateParameterTypeWrapper>,

    /// How does this parameter relates with the parent parameter?
    /// For example for parameter used as `{where: {venue1: {id: {eq: 1}}}}`, we will have following column links:
    /// ```no_rust
    ///   eq: None
    ///   id: Some((<the venues.id column>, None))
    ///   venue1: Some((<the concerts.venue1_id column>, <the venues.id column>))
    ///   where: None
    /// ```
    pub column_path_link: Option<ColumnPathLink>,
    pub access: Option<Access>,
    // TODO: Generalize this to support more than just vector distance functions
    pub vector_distance_function: Option<VectorDistanceFunction>,
}

/// Thw wrapper around PredicateParameterType to be able to satisfy the Named trait, without cloning the parameter type.
/// This one provides a name for the parameter type, while holding to a pointer to the actual parameter type.
/// This is needed because the parameter type is stored in a slab, and we need to be able to get the name of the parameter type
/// without access to the subsystem that holds the slab.
#[derive(Serialize, Deserialize, Debug)]
pub struct PredicateParameterTypeWrapper {
    pub name: String,
    /// Type id of the parameter type. For example: IntFilter, StringFilter, etc.
    pub type_id: SerializableSlabIndex<PredicateParameterType>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PredicateParameterType {
    /// The name of the type. For example, "ConcertFilter", "IntFilter".
    pub name: String,
    pub kind: PredicateParameterTypeKind,
    pub underlying_type: Option<SerializableSlabIndex<EntityType>>,
}

impl Named for PredicateParameterTypeWrapper {
    fn name(&self) -> &str {
        &self.name
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub enum PredicateParameterTypeKind {
    ImplicitEqual,                     // {id: 3}
    Operator(Vec<PredicateParameter>), // {lt: ..,gt: ..} such as IntFilter
    Vector, // {similar: <vector-value>, distance: {<operator such as lt/gt>: <value>}}
    Composite {
        field_params: Vec<PredicateParameter>, // {where: {id: .., name: ..}} such as AccountFilter
        logical_op_params: Vec<PredicateParameter>, // logical operator predicates like `and: [{name: ..}, {id: ..}]`
    },
    Reference(Vec<PredicateParameter>), // {venue: {id: 3}}
}

impl Parameter for PredicateParameter {
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
