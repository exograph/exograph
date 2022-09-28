use async_graphql_parser::{
    types::{FieldDefinition, TypeDefinition},
    Positioned,
};
use payas_core_model::{
    context_type::ContextType,
    mapped_arena::{MappedArena, SerializableSlab},
    type_normalization::{default_positioned, FieldDefinitionProvider, TypeDefinitionProvider},
};
use serde::{Deserialize, Serialize};

use crate::{
    interceptor::Interceptor,
    operation::{DenoMutation, DenoQuery},
    service::ServiceMethod,
    types::ServiceType,
};

use super::service::Script;

#[derive(Serialize, Deserialize, Debug)]
pub struct ModelDenoSystem {
    pub contexts: MappedArena<ContextType>,
    pub service_types: SerializableSlab<ServiceType>,

    // query related
    pub queries: MappedArena<DenoQuery>,

    // mutation related
    pub mutations: MappedArena<DenoMutation>,

    // service related
    pub methods: SerializableSlab<ServiceMethod>,
    pub scripts: SerializableSlab<Script>,
    pub interceptors: SerializableSlab<Interceptor>,
}

impl ModelDenoSystem {
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
        self.service_types
            .iter()
            .map(|typ| typ.1.type_definition(&self.service_types))
            .collect()
    }
}