// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use crate::operation::{OperationParameters, PostgresOperation};
use crate::query::CollectionQueryParameters;
use crate::subsystem::PostgresGraphQLSubsystem;
use async_graphql_parser::types::{
    FieldDefinition, InputObjectType, ObjectType, TypeDefinition, TypeKind,
};
use core_model::access::AccessPredicateExpression;
use core_model::primitive_type::vector_introspection_base_type;
use core_model::type_normalization::Type;
use core_model::types::{DirectivesProvider, TypeValidation};
use core_model::{
    mapped_arena::SerializableSlabIndex,
    type_normalization::{
        default_positioned, default_positioned_name, FieldDefinitionProvider, InputValueProvider,
        Parameter, TypeDefinitionProvider,
    },
    types::{FieldType, Named},
};
use postgres_core_model::relation::OneToManyRelation;
use postgres_core_model::relation::PostgresRelation;

use postgres_core_model::access::DatabaseAccessPrimitiveExpression;

use postgres_core_model::types::{EntityType, PostgresField, PostgresPrimitiveType};
use serde::{Deserialize, Serialize};

/// Mutation input types such as `CreatePostInput` and `UpdatePostInput`
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MutationType {
    pub name: String,
    pub fields: Vec<PostgresField<MutationType>>,
    pub entity_id: SerializableSlabIndex<EntityType>,
    pub doc_comments: Option<String>,

    pub database_access:
        Option<SerializableSlabIndex<AccessPredicateExpression<DatabaseAccessPrimitiveExpression>>>,
}

impl TypeDefinitionProvider<PostgresGraphQLSubsystem> for PostgresPrimitiveType {
    fn type_definition(&self, _system: &PostgresGraphQLSubsystem) -> TypeDefinition {
        TypeDefinition {
            extend: false,
            description: None,
            name: default_positioned_name(&self.name),
            directives: vec![],
            kind: TypeKind::Scalar,
        }
    }
}

impl TypeDefinitionProvider<PostgresGraphQLSubsystem> for EntityType {
    fn type_definition(&self, system: &PostgresGraphQLSubsystem) -> TypeDefinition {
        let EntityType {
            fields,
            agg_fields,
            vector_distance_fields,
            ..
        } = self;

        let kind = {
            let entity = fields
                .iter()
                .map(|field| default_positioned(field.field_definition(system)));

            let agg_fields = agg_fields
                .iter()
                .map(|field| default_positioned(field.field_definition(system)));

            let vector_distance_fields = vector_distance_fields
                .iter()
                .map(|field| default_positioned(field.field_definition(system)));

            let fields = entity
                .chain(agg_fields)
                .chain(vector_distance_fields)
                .collect();

            TypeKind::Object(ObjectType {
                implements: vec![],
                fields,
            })
        };
        TypeDefinition {
            extend: false,
            description: self.doc_comments.clone().map(default_positioned),
            name: default_positioned_name(&self.name),
            directives: vec![],
            kind,
        }
    }
}

impl TypeDefinitionProvider<PostgresGraphQLSubsystem> for MutationType {
    fn type_definition(&self, _system: &PostgresGraphQLSubsystem) -> TypeDefinition {
        let kind = {
            let fields = self
                .fields
                .iter()
                .flat_map(|field| {
                    (!field.readonly).then_some(default_positioned(
                        PostgresMutationField(field).input_value(),
                    ))
                })
                .collect();
            TypeKind::InputObject(InputObjectType { fields })
        };
        TypeDefinition {
            extend: false,
            description: self.doc_comments.clone().map(default_positioned),
            name: default_positioned_name(&self.name),
            directives: vec![],
            kind,
        }
    }
}

impl<CT> FieldDefinitionProvider<PostgresGraphQLSubsystem> for PostgresField<CT> {
    fn field_definition(&self, system: &PostgresGraphQLSubsystem) -> FieldDefinition {
        let mut directives = vec![];

        if let Some(type_validation) = &self.type_validation {
            directives = type_validation
                .get_directives()
                .iter()
                .map(|d| default_positioned(d.to_owned()))
                .collect();
        }

        // Special case for Vector. Even though it is a "scalar" from the perspective of the
        // database, it is a list of floats from the perspective of the GraphQL schema.
        // TODO: This should be handled in a more general way (probably best done with https://github.com/exograph/exograph/issues/603)
        if self.typ.base_type().name() == "Vector" {
            let base_list_type = vector_introspection_base_type();

            return FieldDefinition {
                description: self.doc_comments.clone().map(default_positioned),
                name: default_positioned_name(&self.name),
                arguments: vec![],
                ty: default_positioned(
                    Type {
                        base: base_list_type,
                        nullable: matches!(self.typ, FieldType::Optional(_)),
                    }
                    .to_graphql_type(),
                ),
                directives,
            };
        }

        let arguments = match self.relation {
            PostgresRelation::Scalar { .. }
            | PostgresRelation::ManyToOne { .. }
            | PostgresRelation::Embedded => {
                vec![]
            }
            PostgresRelation::OneToMany(OneToManyRelation {
                foreign_entity_id, ..
            }) => {
                let collection_query = system.get_collection_query(foreign_entity_id);

                let CollectionQueryParameters {
                    predicate_param,
                    order_by_param,
                    limit_param,
                    offset_param,
                } = &collection_query.parameters;

                [
                    predicate_param.input_value(),
                    order_by_param.input_value(),
                    limit_param.input_value(),
                    offset_param.input_value(),
                ]
                .into_iter()
                .map(default_positioned)
                .collect()
            }
        };

        let field_type: Type = (&self.typ).into();
        let field_type = default_positioned(field_type.to_graphql_type());

        FieldDefinition {
            description: self.doc_comments.clone().map(default_positioned),
            name: default_positioned_name(&self.name),
            arguments,
            ty: field_type,
            directives,
        }
    }
}

// To get around the orphan rule, we wrap the field in a struct
struct PostgresMutationField<'a>(&'a PostgresField<MutationType>);

impl<'a> Parameter for PostgresMutationField<'a> {
    fn name(&self) -> &str {
        &self.0.name
    }

    fn typ(&self) -> Type {
        (&self.0.typ).into()
    }

    fn type_validation(&self) -> Option<TypeValidation> {
        self.0.type_validation.clone()
    }
}

pub trait Operation {
    fn name(&self) -> &String;
    fn parameters(&self) -> Vec<&dyn Parameter>;
    fn return_type(&self) -> Type;
    fn doc_comments(&self) -> Option<String>;
}

impl<S, P: OperationParameters> FieldDefinitionProvider<S> for PostgresOperation<P> {
    fn field_definition(&self, _system: &S) -> FieldDefinition {
        let fields = self
            .parameters()
            .iter()
            .map(|parameter| default_positioned(parameter.input_value()))
            .collect();

        FieldDefinition {
            description: self.doc_comments.clone().map(default_positioned),
            name: default_positioned_name(self.name()),
            arguments: fields,
            directives: vec![],
            ty: default_positioned(self.return_type().to_graphql_type()),
        }
    }
}
