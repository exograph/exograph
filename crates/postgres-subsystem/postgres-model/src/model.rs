use std::vec;

use async_graphql_parser::types::{FieldDefinition, TypeDefinition};

use crate::operation::CollectionQuery;

use super::{
    operation::{PkQuery, PostgresMutation},
    order::OrderByParameterType,
    predicate::PredicateParameterType,
    types::PostgresType,
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
pub struct ModelPostgresSystem {
    pub contexts: MappedArena<ContextType>,
    pub postgres_types: SerializableSlab<PostgresType>,

    // query related
    pub order_by_types: SerializableSlab<OrderByParameterType>,
    pub predicate_types: SerializableSlab<PredicateParameterType>,
    pub pk_queries: MappedArena<PkQuery>,
    pub collection_queries: MappedArena<CollectionQuery>,

    // mutation related
    pub mutation_types: SerializableSlab<PostgresType>, // create, update, delete input types such as `PersonUpdateInput`
    pub mutations: MappedArena<PostgresMutation>,

    pub tables: SerializableSlab<PhysicalTable>,
}

impl ModelPostgresSystem {
    pub fn schema_queries(&self) -> Vec<FieldDefinition> {
        let pk_queries_defn = self
            .pk_queries
            .iter()
            .map(|(_, query)| query.field_definition(self));

        let collection_queries_defn = self
            .collection_queries
            .iter()
            .map(|(_, query)| query.field_definition(self));

        pk_queries_defn.chain(collection_queries_defn).collect()
    }

    pub fn schema_mutations(&self) -> Vec<FieldDefinition> {
        self.mutations
            .iter()
            .map(|(_, mutation)| mutation.field_definition(self))
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
            pk_queries: MappedArena::default(),
            collection_queries: MappedArena::default(),
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
