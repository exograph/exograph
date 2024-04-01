// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use crate::{access::Access, subsystem::PostgresSubsystem};

use async_graphql_parser::{
    types::{
        BaseType, EnumType, EnumValueDefinition, InputObjectType, InputValueDefinition, Type,
        TypeDefinition, TypeKind,
    },
    Pos, Positioned,
};
use async_graphql_value::Name;
use core_plugin_interface::core_model::{
    mapped_arena::SerializableSlabIndex,
    primitive_type::vector_introspection_type,
    type_normalization::{
        default_positioned, default_positioned_name, InputValueProvider, Parameter,
        TypeDefinitionProvider,
    },
    types::{FieldType, Named},
};

use exo_sql::{ColumnPathLink, VectorDistanceFunction};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct OrderByParameter {
    pub name: String,
    pub typ: FieldType<OrderByParameterTypeWrapper>,

    /// How does this parameter relates with the parent parameter?
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

#[derive(Serialize, Deserialize, Debug)]
pub struct OrderByParameterTypeWrapper {
    pub name: String,
    pub type_id: SerializableSlabIndex<OrderByParameterType>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct OrderByParameterType {
    pub name: String,
    pub kind: OrderByParameterTypeKind,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum OrderByParameterTypeKind {
    Primitive,
    Vector,
    Composite { parameters: Vec<OrderByParameter> },
}

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
}

impl TypeDefinitionProvider<PostgresSubsystem> for OrderByParameterType {
    fn type_definition(&self, _system: &PostgresSubsystem) -> TypeDefinition {
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
                        ty: default_positioned(vector_introspection_type(false)),
                    },
                    InputValueDefinition {
                        description: None,
                        name: default_positioned_name("order"),
                        directives: vec![],
                        default_value: None,
                        ty: default_positioned(Type {
                            base: BaseType::Named(Name::new("Ordering")),
                            nullable: true,
                        }),
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

impl TypeDefinitionProvider<PostgresSubsystem> for OrderByParameterTypeWrapper {
    fn type_definition(&self, system: &PostgresSubsystem) -> TypeDefinition {
        let typ = &system.order_by_types[self.type_id];
        typ.type_definition(system)
    }
}
