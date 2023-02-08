use async_graphql_parser::{
    types::{FieldDefinition, TypeDefinition},
    Positioned,
};
use core_model::{
    context_type::ContextType,
    mapped_arena::{MappedArena, SerializableSlab},
    type_normalization::{default_positioned, FieldDefinitionProvider, TypeDefinitionProvider},
};
use serde::{Deserialize, Serialize};

use crate::{
    interceptor::Interceptor,
    operation::{ServiceMutation, ServiceQuery},
    service::ServiceMethod,
    types::ServiceType,
};

use super::service::Script;

#[derive(Serialize, Deserialize, Debug)]
pub struct ServiceSubsystem {
    pub contexts: MappedArena<ContextType>,
    pub service_types: SerializableSlab<ServiceType>,

    // query related
    pub queries: MappedArena<ServiceQuery>,

    // mutation related
    pub mutations: MappedArena<ServiceMutation>,

    // service related
    pub methods: SerializableSlab<ServiceMethod>,
    pub scripts: SerializableSlab<Script>,
    pub interceptors: SerializableSlab<Interceptor>,
}

impl ServiceSubsystem {
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
