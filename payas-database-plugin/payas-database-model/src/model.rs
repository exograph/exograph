use std::vec;

use async_graphql_parser::{
    types::{FieldDefinition, TypeDefinition},
    Positioned,
};
use payas_core_model::{
    context_type::ContextType,
    error::ModelSerializationError,
    mapped_arena::{MappedArena, SerializableSlab},
    system_serializer::SystemSerializer,
    type_normalization::{default_positioned, FieldDefinitionProvider, TypeDefinitionProvider},
};
use payas_sql::PhysicalTable;
use serde::{Deserialize, Serialize};

use super::{
    operation::{DatabaseMutation, DatabaseQuery},
    order::OrderByParameterType,
    predicate::PredicateParameterType,
    types::DatabaseType,
};

#[derive(Serialize, Deserialize, Debug)]
pub struct ModelDatabaseSystem {
    pub contexts: MappedArena<ContextType>,
    pub database_types: SerializableSlab<DatabaseType>,

    // query related
    pub order_by_types: SerializableSlab<OrderByParameterType>,
    pub predicate_types: SerializableSlab<PredicateParameterType>,
    pub queries: MappedArena<DatabaseQuery>,

    // mutation related
    pub mutation_types: SerializableSlab<DatabaseType>, // create, update, delete input types such as `PersonUpdateInput`
    pub mutations: MappedArena<DatabaseMutation>,

    pub tables: SerializableSlab<PhysicalTable>,
}

impl ModelDatabaseSystem {
    pub fn new() -> Self {
        Self {
            contexts: MappedArena::default(),
            database_types: SerializableSlab::new(),
            order_by_types: SerializableSlab::new(),
            predicate_types: SerializableSlab::new(),
            queries: MappedArena::default(),
            mutation_types: SerializableSlab::new(),
            mutations: MappedArena::default(),
            tables: SerializableSlab::new(),
        }
    }

    pub fn schema_queries(&self) -> Vec<Positioned<FieldDefinition>> {
        self.queries
            .values
            .iter()
            .map(|query| default_positioned(query.1.field_definition(self)))
            .collect()
    }

    pub fn schema_mutations(&self) -> Vec<Positioned<FieldDefinition>> {
        self.mutations
            .values
            .iter()
            .map(|query| default_positioned(query.1.field_definition(self)))
            .collect()
    }

    pub fn schema_types(&self) -> Vec<TypeDefinition> {
        let database_type_definitions: Vec<TypeDefinition> = self
            .database_types
            .iter()
            .map(|model_type| model_type.1.type_definition(self))
            .collect();

        let order_by_param_type_definitions: Vec<TypeDefinition> = self
            .order_by_types
            .iter()
            .map(|parameter_type| parameter_type.1.type_definition(self))
            .collect();

        let predicate_param_type_definitions: Vec<TypeDefinition> = self
            .predicate_types
            .iter()
            .map(|parameter_type| parameter_type.1.type_definition(self))
            .collect();

        let mutation_param_type_definitions: Vec<TypeDefinition> = self
            .mutation_types
            .iter()
            .map(|parameter_type| parameter_type.1.type_definition(self))
            .collect();

        let types = vec![
            database_type_definitions.into_iter(),
            order_by_param_type_definitions.into_iter(),
            predicate_param_type_definitions.into_iter(),
            mutation_param_type_definitions.into_iter(),
        ]
        .into_iter()
        .flatten()
        .collect();

        types
    }
}

impl SystemSerializer for ModelDatabaseSystem {
    type Underlying = Self;

    fn serialize(&self) -> Result<Vec<u8>, ModelSerializationError> {
        bincode::serialize(self).map_err(|e| ModelSerializationError::Serialize(e))
    }

    fn deserialize_reader(
        reader: impl std::io::Read,
    ) -> Result<Self::Underlying, ModelSerializationError> {
        bincode::deserialize_from(reader).map_err(|e| ModelSerializationError::Deserialize(e))
    }
}
