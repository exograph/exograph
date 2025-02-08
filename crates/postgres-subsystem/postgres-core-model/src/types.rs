// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use super::relation::PostgresRelation;
use crate::aggregate::AggregateField;
use crate::vector_distance::VectorDistanceField;

use common::value::Val;
use core_plugin_interface::core_model::context_type::ContextSelection;
use core_plugin_interface::core_model::types::TypeValidation;
use core_plugin_interface::core_model::{
    mapped_arena::{SerializableSlab, SerializableSlabIndex},
    types::{FieldType, Named},
};

use crate::access::Access;

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

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
pub enum EntityRepresentation {
    Json,
    Managed,
    NotManaged,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct EntityType {
    pub name: String,
    pub plural_name: String,
    pub representation: EntityRepresentation,

    pub fields: Vec<PostgresField<EntityType>>,
    pub agg_fields: Vec<AggregateField>,
    pub vector_distance_fields: Vec<VectorDistanceField>,

    pub table_id: SerializableSlabIndex<PhysicalTable>,
    pub access: Access,
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

impl EntityType {
    pub fn field_by_name(&self, name: &str) -> Option<&PostgresField<EntityType>> {
        self.fields.iter().find(|field| field.name == name)
    }

    pub fn pk_fields(&self) -> Vec<&PostgresField<EntityType>> {
        self.fields
            .iter()
            .filter(|field| field.relation.is_pk())
            .collect()
    }

    pub fn pk_field_ids(&self, entity_id: SerializableSlabIndex<EntityType>) -> Vec<EntityFieldId> {
        self.fields
            .iter()
            .enumerate()
            .filter(|(_, field)| field.relation.is_pk())
            .map(|(field_index, _)| EntityFieldId(field_index, entity_id))
            .collect()
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
pub enum PostgresFieldDefaultValue {
    Static(Val),
    Dynamic(ContextSelection),
    Function(String), // Postgres function name such as `now()`
    AutoIncrement,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PostgresField<CT> {
    pub name: String,
    pub typ: FieldType<PostgresFieldType<CT>>,
    pub relation: PostgresRelation,
    pub default_value: Option<PostgresFieldDefaultValue>,
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
