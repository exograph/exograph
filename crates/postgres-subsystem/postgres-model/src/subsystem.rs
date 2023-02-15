use std::vec;

use async_graphql_parser::types::{FieldDefinition, TypeDefinition};

use super::{
    operation::{PkQuery, PostgresMutation},
    order::OrderByParameterType,
    predicate::PredicateParameterType,
};
use crate::{
    aggregate::AggregateType,
    operation::{AggregateQuery, CollectionQuery},
    types::{EntityType, MutationType, PostgresPrimitiveType},
};
use core_plugin_interface::{
    core_model::{
        context_type::ContextType,
        mapped_arena::{MappedArena, SerializableSlab},
        type_normalization::{FieldDefinitionProvider, TypeDefinitionProvider},
    },
    error::ModelSerializationError,
    system_serializer::SystemSerializer,
};
use payas_sql::PhysicalTable;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct PostgresSubsystem {
    pub contexts: MappedArena<ContextType>,
    pub primitive_types: SerializableSlab<PostgresPrimitiveType>,
    pub entity_types: SerializableSlab<EntityType>,

    pub aggregate_types: SerializableSlab<AggregateType>,

    // query related
    pub order_by_types: SerializableSlab<OrderByParameterType>,
    pub predicate_types: SerializableSlab<PredicateParameterType>,

    pub pk_queries: MappedArena<PkQuery>,
    pub collection_queries: MappedArena<CollectionQuery>,
    pub aggregate_queries: MappedArena<AggregateQuery>,

    // mutation related
    pub mutation_types: SerializableSlab<MutationType>, // create, update, delete input types such as `PersonUpdateInput`
    pub mutations: MappedArena<PostgresMutation>,

    pub tables: SerializableSlab<PhysicalTable>,
}

impl PostgresSubsystem {
    pub fn schema_queries(&self) -> Vec<FieldDefinition> {
        let pk_queries_defn = self
            .pk_queries
            .iter()
            .map(|(_, query)| query.field_definition(self));

        let collection_queries_defn = self
            .collection_queries
            .iter()
            .map(|(_, query)| query.field_definition(self));

        let aggregate_queries_defn = self
            .aggregate_queries
            .iter()
            .map(|query| query.1.field_definition(self));

        pk_queries_defn
            .chain(collection_queries_defn)
            .chain(aggregate_queries_defn)
            .collect()
    }

    pub fn schema_mutations(&self) -> Vec<FieldDefinition> {
        self.mutations
            .iter()
            .map(|(_, mutation)| mutation.field_definition(self))
            .collect()
    }

    pub fn schema_types(&self) -> Vec<TypeDefinition> {
        let mut all_type_definitions = vec![];

        self.primitive_types
            .iter()
            .for_each(|typ| all_type_definitions.push(typ.1.type_definition(self)));

        self.entity_types
            .iter()
            .for_each(|typ| all_type_definitions.push(typ.1.type_definition(self)));

        self.aggregate_types
            .iter()
            .for_each(|typ| all_type_definitions.push(typ.1.type_definition(self)));

        self.order_by_types.iter().for_each(|parameter_type| {
            all_type_definitions.push(parameter_type.1.type_definition(self))
        });

        self.predicate_types.iter().for_each(|parameter_type| {
            all_type_definitions.push(parameter_type.1.type_definition(self))
        });

        self.mutation_types.iter().for_each(|parameter_type| {
            all_type_definitions.push(parameter_type.1.type_definition(self))
        });

        all_type_definitions
    }
}

impl Default for PostgresSubsystem {
    fn default() -> Self {
        Self {
            contexts: MappedArena::default(),
            primitive_types: SerializableSlab::new(),
            entity_types: SerializableSlab::new(),
            aggregate_types: SerializableSlab::new(),
            order_by_types: SerializableSlab::new(),
            predicate_types: SerializableSlab::new(),
            pk_queries: MappedArena::default(),
            collection_queries: MappedArena::default(),
            aggregate_queries: MappedArena::default(),
            mutation_types: SerializableSlab::new(),
            mutations: MappedArena::default(),
            tables: SerializableSlab::new(),
        }
    }
}

impl SystemSerializer for PostgresSubsystem {
    type Underlying = Self;

    fn serialize(&self) -> Result<Vec<u8>, ModelSerializationError> {
        bincode::serialize(self).map_err(ModelSerializationError::Serialize)
    }

    fn deserialize_reader(
        reader: impl std::io::Read,
    ) -> Result<Self::Underlying, ModelSerializationError> {
        bincode::deserialize_from(reader).map_err(ModelSerializationError::Deserialize)
    }
}
