// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use super::access::Access;
use super::relation::PostgresRelation;
use crate::access::{DatabaseAccessPrimitiveExpression, InputAccessPrimitiveExpression};
use crate::aggregate::AggregateField;
use crate::query::{AggregateQuery, CollectionQuery, CollectionQueryParameters, PkQuery};
use crate::relation::OneToManyRelation;
use crate::subsystem::PostgresSubsystem;
use crate::vector_distance::VectorDistanceField;
use async_graphql_parser::types::{
    FieldDefinition, InputObjectType, ObjectType, Type, TypeDefinition, TypeKind,
};
use core_plugin_interface::core_model::access::AccessPredicateExpression;
use core_plugin_interface::core_model::context_type::ContextSelection;
use core_plugin_interface::core_model::primitive_type::vector_introspection_base_type;
use core_plugin_interface::core_model::types::{DirectivesProvider, TypeValidation};
use core_plugin_interface::core_model::{
    mapped_arena::{SerializableSlab, SerializableSlabIndex},
    type_normalization::{
        default_positioned, default_positioned_name, FieldDefinitionProvider, InputValueProvider,
        Parameter, TypeDefinitionProvider,
    },
    types::{FieldType, Named},
};
use exo_sql::PhysicalTable;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub enum TypeIndex<CT> {
    Primitive(SerializableSlabIndex<PostgresPrimitiveType>),
    Composite(SerializableSlabIndex<CT>),
}

impl<CT> TypeIndex<CT> {
    pub fn to_type<'a>(
        &self,
        primitive_types: &'a SerializableSlab<PostgresPrimitiveType>,
        entity_types: &'a SerializableSlab<CT>,
    ) -> PostgresType<'a, CT> {
        match self {
            TypeIndex::Primitive(index) => PostgresType::Primitive(&primitive_types[*index]),
            TypeIndex::Composite(index) => PostgresType::Composite(&entity_types[*index]),
        }
    }
}

#[derive(Debug)]
pub enum PostgresType<'a, CT> {
    Primitive(&'a PostgresPrimitiveType),
    Composite(&'a CT),
}

impl<'a, CT: Named> PostgresType<'a, CT> {
    pub fn name(&self) -> &str {
        match self {
            PostgresType::Primitive(primitive_type) => &primitive_type.name,
            PostgresType::Composite(composite_type) => composite_type.name(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PostgresPrimitiveType {
    pub name: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct EntityType {
    pub name: String,
    pub plural_name: String,

    pub fields: Vec<PostgresField<EntityType>>,
    pub agg_fields: Vec<AggregateField>,
    pub vector_distance_fields: Vec<VectorDistanceField>,
    pub table_id: SerializableSlabIndex<PhysicalTable>,
    pub pk_query: SerializableSlabIndex<PkQuery>,
    pub collection_query: SerializableSlabIndex<CollectionQuery>,
    pub aggregate_query: SerializableSlabIndex<AggregateQuery>,
    pub access: Access,
}

pub fn get_field_id(
    types: &SerializableSlab<EntityType>,
    entity_id: SerializableSlabIndex<EntityType>,
    field_name: &str,
) -> Option<EntityFieldId> {
    let entity = &types[entity_id];
    entity
        .fields
        .iter()
        .position(|field| field.name == field_name)
        .map(|field_index| EntityFieldId(field_index, entity_id))
}

/// Encapsulates a field on an entity type (mirros how `ColumnId` is structured)
#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub struct EntityFieldId(usize, SerializableSlabIndex<EntityType>);

impl EntityFieldId {
    pub fn entity_type_id(&self) -> SerializableSlabIndex<EntityType> {
        self.1
    }

    pub fn resolve<'a>(
        &self,
        types: &'a SerializableSlab<EntityType>,
    ) -> &'a PostgresField<EntityType> {
        let entity = &types[self.1];
        &entity.fields[self.0]
    }
}

impl Named for EntityType {
    fn name(&self) -> &str {
        &self.name
    }
}

/// Mutation input types such as `CreatePostInput` and `UpdatePostInput`
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MutationType {
    pub name: String,
    pub fields: Vec<PostgresField<MutationType>>,
    pub entity_id: SerializableSlabIndex<EntityType>,

    pub input_access:
        Option<SerializableSlabIndex<AccessPredicateExpression<InputAccessPrimitiveExpression>>>,
    pub database_access:
        Option<SerializableSlabIndex<AccessPredicateExpression<DatabaseAccessPrimitiveExpression>>>,
}

impl EntityType {
    pub fn field_by_name(&self, name: &str) -> Option<&PostgresField<EntityType>> {
        self.fields.iter().find(|field| field.name == name)
    }

    pub fn pk_field(&self) -> Option<&PostgresField<EntityType>> {
        self.fields
            .iter()
            .find(|field| matches!(&field.relation, PostgresRelation::Pk { .. }))
    }

    pub fn pk_field_id(
        &self,
        entity_id: SerializableSlabIndex<EntityType>,
    ) -> Option<EntityFieldId> {
        self.fields
            .iter()
            .position(|field| matches!(&field.relation, PostgresRelation::Pk { .. }))
            .map(|field_index| EntityFieldId(field_index, entity_id))
    }

    pub fn aggregate_field_by_name(&self, name: &str) -> Option<&AggregateField> {
        self.agg_fields.iter().find(|field| field.name == name)
    }

    pub fn vector_distance_field_by_name(&self, name: &str) -> Option<&VectorDistanceField> {
        self.vector_distance_fields
            .iter()
            .find(|field| field.name == name)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PostgresField<CT> {
    pub name: String,
    pub typ: FieldType<PostgresFieldType<CT>>,
    pub relation: PostgresRelation,
    pub has_default_value: bool, // does this field have a default value?
    pub dynamic_default_value: Option<ContextSelection>,
    pub readonly: bool,
    pub access: Access,
    pub type_validation: Option<TypeValidation>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PostgresFieldType<CT> {
    pub type_id: TypeIndex<CT>,
    pub type_name: String,
}

impl<CT> Named for PostgresFieldType<CT> {
    fn name(&self) -> &str {
        &self.type_name
    }
}

pub fn base_type<'a, CT>(
    typ: &FieldType<PostgresFieldType<CT>>,
    primitive_types: &'a SerializableSlab<PostgresPrimitiveType>,
    entity_types: &'a SerializableSlab<CT>,
) -> PostgresType<'a, CT> {
    typ.innermost()
        .type_id
        .to_type(primitive_types, entity_types)
}

impl TypeDefinitionProvider<PostgresSubsystem> for PostgresPrimitiveType {
    fn type_definition(&self, _system: &PostgresSubsystem) -> TypeDefinition {
        TypeDefinition {
            extend: false,
            description: None,
            name: default_positioned_name(&self.name),
            directives: vec![],
            kind: TypeKind::Scalar,
        }
    }
}

impl TypeDefinitionProvider<PostgresSubsystem> for EntityType {
    fn type_definition(&self, system: &PostgresSubsystem) -> TypeDefinition {
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
            description: None,
            name: default_positioned_name(&self.name),
            directives: vec![],
            kind,
        }
    }
}

impl TypeDefinitionProvider<PostgresSubsystem> for MutationType {
    fn type_definition(&self, _system: &PostgresSubsystem) -> TypeDefinition {
        let kind = {
            let fields = self
                .fields
                .iter()
                .flat_map(|field| {
                    (!field.readonly).then_some(default_positioned(field.input_value()))
                })
                .collect();
            TypeKind::InputObject(InputObjectType { fields })
        };
        TypeDefinition {
            extend: false,
            description: None,
            name: default_positioned_name(&self.name),
            directives: vec![],
            kind,
        }
    }
}

impl<CT> FieldDefinitionProvider<PostgresSubsystem> for PostgresField<CT> {
    fn field_definition(&self, system: &PostgresSubsystem) -> FieldDefinition {
        let field_type = default_positioned((&self.typ).into());
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
                description: None,
                name: default_positioned_name(&self.name),
                arguments: vec![],
                ty: default_positioned(Type {
                    base: base_list_type,
                    nullable: matches!(self.typ, FieldType::Optional(_)),
                }),
                directives,
            };
        }

        let arguments = match self.relation {
            PostgresRelation::Pk { .. }
            | PostgresRelation::Scalar { .. }
            | PostgresRelation::ManyToOne { .. } => {
                vec![]
            }
            PostgresRelation::OneToMany(OneToManyRelation {
                foreign_field_id, ..
            }) => {
                let foreign_type = &system.entity_types[foreign_field_id.entity_type_id()];
                let collection_query = &system.collection_queries[foreign_type.collection_query];

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

        FieldDefinition {
            description: None,
            name: default_positioned_name(&self.name),
            arguments,
            ty: field_type,
            directives,
        }
    }
}

impl<CT> Parameter for PostgresField<CT> {
    fn name(&self) -> &str {
        &self.name
    }

    fn typ(&self) -> Type {
        (&self.typ).into()
    }

    fn type_validation(&self) -> Option<TypeValidation> {
        self.type_validation.clone()
    }
}
