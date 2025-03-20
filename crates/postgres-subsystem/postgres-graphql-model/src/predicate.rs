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
use core_model::{
    primitive_type::vector_introspection_type,
    type_normalization::{
        default_positioned, default_positioned_name, InputValueProvider, TypeDefinitionProvider,
    },
};

use postgres_core_model::predicate::{PredicateParameterType, PredicateParameterTypeKind};

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
                        ty: default_positioned(vector_introspection_type(false).to_graphql_type()),
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
