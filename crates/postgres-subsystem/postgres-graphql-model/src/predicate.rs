// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use crate::subsystem::PostgresGraphQLSubsystem;
use async_graphql_parser::types::{
    InputObjectType, InputValueDefinition, Type, TypeDefinition, TypeKind,
};
use core_plugin_interface::core_model::{
    mapped_arena::SerializableSlabIndex,
    primitive_type::vector_introspection_type,
    type_normalization::{
        default_positioned, default_positioned_name, InputValueProvider, Parameter,
        TypeDefinitionProvider,
    },
    types::{FieldType, Named, TypeValidation},
};

use postgres_core_model::{access::Access, types::EntityType};

use exo_sql::{ColumnPathLink, VectorDistanceFunction};
use serde::{Deserialize, Serialize};

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

impl TypeDefinitionProvider<PostgresGraphQLSubsystem> for PredicateParameterType {
    fn type_definition(&self, system: &PostgresGraphQLSubsystem) -> TypeDefinition {
        match &self.kind {
            PredicateParameterTypeKind::Operator(parameters)
            | PredicateParameterTypeKind::Reference(parameters) => {
                let fields = parameters
                    .iter()
                    .map(|parameter| default_positioned(parameter.input_value()))
                    .collect();
                let description = self
                    .underlying_type
                    .map(|underlying_type| &system.core_subsystem.entity_types[underlying_type])
                    .map(|entity_type| {
                        format!(
                            "A predicate to filter the results for a `{}` type parameter.",
                            entity_type.name.clone()
                        )
                    });
                TypeDefinition {
                    extend: false,
                    description: description.map(default_positioned),
                    name: default_positioned_name(&self.name),
                    directives: vec![],
                    kind: TypeKind::InputObject(InputObjectType { fields }),
                }
            }
            PredicateParameterTypeKind::Composite {
                field_params,
                logical_op_params,
            } => {
                let parameters = field_params.iter().chain(logical_op_params.iter());

                let fields = parameters
                    .map(|parameter| default_positioned(parameter.input_value()))
                    .collect();

                let description = self
                    .underlying_type
                    .map(|underlying_type| &system.core_subsystem.entity_types[underlying_type])
                    .map(|entity_type| {
                        format!(
                            "Predicate for the `{}` type parameter. 
If a field is omitted, no filter is applied for that field.
To check a field against null, use a `<field name>: null` filter",
                            entity_type.name.clone()
                        )
                    });
                TypeDefinition {
                    extend: false,
                    description: description.map(default_positioned),
                    name: default_positioned_name(&self.name),
                    directives: vec![],
                    kind: TypeKind::InputObject(InputObjectType { fields }),
                }
            }
            PredicateParameterTypeKind::ImplicitEqual => TypeDefinition {
                extend: false,
                description: Some(default_positioned(
                    "A single value to match against using the equal operator.".to_string(),
                )),
                name: default_positioned_name(&self.name),
                directives: vec![],
                kind: TypeKind::Scalar,
            },
            PredicateParameterTypeKind::Vector => {
                let fields = vec![
                    InputValueDefinition {
                        description: Some(default_positioned(
                            "The target vector to compare against.".to_string(),
                        )),
                        name: default_positioned_name("distanceTo"),
                        ty: default_positioned(vector_introspection_type(false)),
                        default_value: None,
                        directives: vec![],
                    },
                    InputValueDefinition {
                        description: Some(default_positioned(
                            "The distance to the vector.".to_string(),
                        )),
                        name: default_positioned_name("distance"),
                        ty: default_positioned(Type::new("FloatFilter").unwrap()),
                        default_value: None,
                        directives: vec![],
                    },
                ]
                .into_iter()
                .map(default_positioned)
                .collect();

                TypeDefinition {
                    extend: false,
                    description: Some(default_positioned(
                        "Predicate to filter based on vector distance".to_string(),
                    )),
                    name: default_positioned_name(&self.name),
                    directives: vec![],
                    kind: TypeKind::InputObject(InputObjectType { fields }),
                }
            }
        }
    }
}
