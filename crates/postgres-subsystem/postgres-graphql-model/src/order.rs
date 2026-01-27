// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

//! GraphQL-specific trait implementations for order-by types.

use crate::subsystem::PostgresGraphQLSubsystem;

use async_graphql_parser::{
    Pos, Positioned,
    types::{
        EnumType, EnumValueDefinition, InputObjectType, InputValueDefinition, TypeDefinition,
        TypeKind,
    },
};
use async_graphql_value::Name;
use core_model::{
    primitive_type::vector_introspection_type,
    type_normalization::{
        BaseType, InputValueProvider, Type, TypeDefinitionProvider, default_positioned,
        default_positioned_name,
    },
};
use postgres_core_model::order::{
    OrderByParameterType, OrderByParameterTypeKind, OrderByParameterTypeWrapper,
    PRIMITIVE_ORDERING_OPTIONS,
};

impl TypeDefinitionProvider<PostgresGraphQLSubsystem> for OrderByParameterType {
    fn type_definition(&self, _system: &PostgresGraphQLSubsystem) -> TypeDefinition {
        match &self.kind {
            OrderByParameterTypeKind::Composite { parameters } => {
                let fields = parameters
                    .iter()
                    .map(|parameter| default_positioned(parameter.input_value()))
                    .collect();

                TypeDefinition {
                    extend: false,
                    description: None,
                    name: default_positioned_name(&self.name),
                    directives: vec![],
                    kind: TypeKind::InputObject(InputObjectType { fields }),
                }
            }
            OrderByParameterTypeKind::Primitive => TypeDefinition {
                extend: false,
                description: None,
                name: default_positioned_name(&self.name),
                directives: vec![],
                kind: TypeKind::Enum(EnumType {
                    values: PRIMITIVE_ORDERING_OPTIONS
                        .iter()
                        .map(|value| {
                            Positioned::new(
                                EnumValueDefinition {
                                    description: None,
                                    value: Positioned::new(Name::new(value), Pos::default()),
                                    directives: vec![],
                                },
                                Pos::default(),
                            )
                        })
                        .collect(),
                }),
            },
            OrderByParameterTypeKind::Vector => {
                let fields = vec![
                    InputValueDefinition {
                        description: None,
                        name: default_positioned_name("distanceTo"),
                        directives: vec![],
                        default_value: None,
                        ty: default_positioned(vector_introspection_type(false).to_graphql_type()),
                    },
                    InputValueDefinition {
                        description: None,
                        name: default_positioned_name("order"),
                        directives: vec![],
                        default_value: None,
                        ty: default_positioned(
                            Type {
                                base: BaseType::Leaf("Ordering".to_string()),
                                nullable: true,
                            }
                            .to_graphql_type(),
                        ),
                    },
                ]
                .into_iter()
                .map(default_positioned)
                .collect();

                TypeDefinition {
                    extend: false,
                    description: None,
                    name: default_positioned_name(&self.name),
                    directives: vec![],
                    kind: TypeKind::InputObject(InputObjectType { fields }),
                }
            }
        }
    }
}

impl TypeDefinitionProvider<PostgresGraphQLSubsystem> for OrderByParameterTypeWrapper {
    fn type_definition(&self, system: &PostgresGraphQLSubsystem) -> TypeDefinition {
        let typ = &system.core_subsystem.order_by_types[self.type_id];
        typ.type_definition(system)
    }
}
