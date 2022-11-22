use std::vec;

use async_graphql_parser::{
    types::{FieldDefinition, TypeDefinition},
    Positioned,
};

use super::{
    operation::{PostgresMutation, PostgresQuery},
    order::OrderByParameterType,
    predicate::PredicateParameterType,
    types::PostgresType,
};
use core_plugin_interface::{
    core_model::{
        context_type::ContextType,
        mapped_arena::{MappedArena, SerializableSlab},
        type_normalization::{default_positioned, FieldDefinitionProvider, TypeDefinitionProvider},
    },
    error::ModelSerializationError,
    system_serializer::SystemSerializer,
};
use payas_sql::PhysicalTable;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct ModelPostgresSystem {
    pub contexts: MappedArena<ContextType>,
    pub postgres_types: SerializableSlab<PostgresType>,

    // query related
    pub order_by_types: SerializableSlab<OrderByParameterType>,
    pub predicate_types: SerializableSlab<PredicateParameterType>,
    pub queries: MappedArena<PostgresQuery>,

    // mutation related
    pub mutation_types: SerializableSlab<PostgresType>, // create, update, delete input types such as `PersonUpdateInput`
    pub mutations: MappedArena<PostgresMutation>,

    pub tables: SerializableSlab<PhysicalTable>,
}

impl ModelPostgresSystem {
    pub fn schema_queries(&self) -> Vec<Positioned<FieldDefinition>> {
        self.queries
            .iter()
            .map(|query| default_positioned(query.1.field_definition(self)))
            .collect()
    }

    pub fn schema_mutations(&self) -> Vec<Positioned<FieldDefinition>> {
        self.mutations
            .iter()
            .map(|mutation| default_positioned(mutation.1.field_definition(self)))
            .collect()
    }

    pub fn schema_types(&self) -> Vec<TypeDefinition> {
        let mut all_type_definitions = vec![];

        self.postgres_types
            .iter()
            .for_each(|model_type| all_type_definitions.push(model_type.1.type_definition(self)));

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

impl Default for ModelPostgresSystem {
    fn default() -> Self {
        Self {
            contexts: MappedArena::default(),
            postgres_types: SerializableSlab::new(),
            order_by_types: SerializableSlab::new(),
            predicate_types: SerializableSlab::new(),
            queries: MappedArena::default(),
            mutation_types: SerializableSlab::new(),
            mutations: MappedArena::default(),
            tables: SerializableSlab::new(),
        }
    }
}

impl SystemSerializer for ModelPostgresSystem {
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
