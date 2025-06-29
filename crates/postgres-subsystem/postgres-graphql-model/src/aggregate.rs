// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use async_graphql_parser::types::{
    BaseType, FieldDefinition, ObjectType, Type, TypeDefinition, TypeKind,
};
use async_graphql_value::Name;
use postgres_core_model::aggregate::{AggregateField, AggregateFieldType, AggregateType};

use crate::query::AggregateQueryParameters;
use crate::subsystem::PostgresGraphQLSubsystem;
use core_model::type_normalization::{
    FieldDefinitionProvider, InputValueProvider, TypeDefinitionProvider, default_positioned,
    default_positioned_name,
};
use postgres_core_model::relation::{OneToManyRelation, PostgresRelation};

impl TypeDefinitionProvider<PostgresGraphQLSubsystem> for AggregateType {
    fn type_definition(&self, system: &PostgresGraphQLSubsystem) -> TypeDefinition {
        let kind = {
            let fields: Vec<_> = self
                .fields
                .iter()
                .map(|field| default_positioned(field.field_definition(system)))
                .collect();

            TypeKind::Object(ObjectType {
                implements: vec![],
                fields,
            })
        };
        TypeDefinition {
            extend: false,
            description: system
                .core_subsystem
                .entity_types
                .get(self.underlying_type)
                .map(|entity_type| {
                    default_positioned(format!("An aggregate for the `{}` type.", entity_type.name))
                }),
            name: default_positioned_name(&self.name),
            directives: vec![],
            kind,
        }
    }
}

impl FieldDefinitionProvider<PostgresGraphQLSubsystem> for AggregateField {
    fn field_definition(&self, system: &PostgresGraphQLSubsystem) -> FieldDefinition {
        let arguments = match &self.relation {
            Some(relation) => match relation {
                PostgresRelation::Scalar { .. } | PostgresRelation::ManyToOne { .. } => {
                    vec![]
                }
                PostgresRelation::OneToMany(OneToManyRelation {
                    foreign_entity_id, ..
                }) => {
                    let aggregate_query = system.get_aggregate_query(*foreign_entity_id);

                    let AggregateQueryParameters { predicate_param } = &aggregate_query.parameters;

                    vec![default_positioned(predicate_param.input_value())]
                }
                PostgresRelation::Embedded => {
                    vec![]
                }
            },
            None => vec![],
        };

        FieldDefinition {
            description: None,
            name: default_positioned_name(&self.name),
            arguments,
            ty: default_positioned(compute_type(&self.typ)),
            directives: vec![],
        }
    }
}

fn compute_type(typ: &AggregateFieldType) -> Type {
    let base = match typ {
        AggregateFieldType::Scalar { type_name, .. } => BaseType::Named(Name::new(type_name)),
        AggregateFieldType::Composite { type_name, .. } => BaseType::Named(Name::new(type_name)),
    };

    Type {
        base,
        nullable: true,
    }
}
