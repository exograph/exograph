use super::service::Script;
use crate::{
    interceptor::Interceptor,
    operation::{WasmMutation, WasmQuery},
    service::ServiceMethod,
    types::ServiceType,
};
use async_graphql_parser::types::{FieldDefinition, TypeDefinition};
use core_plugin_interface::{
    core_model::{
        context_type::ContextType,
        mapped_arena::{MappedArena, SerializableSlab},
        type_normalization::{FieldDefinitionProvider, TypeDefinitionProvider},
    },
    error::ModelSerializationError,
    system_serializer::SystemSerializer,
};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct ModelWasmSystem {
    pub contexts: MappedArena<ContextType>,
    pub service_types: SerializableSlab<ServiceType>,

    // query related
    pub queries: MappedArena<WasmQuery>,

    // mutation related
    pub mutations: MappedArena<WasmMutation>,

    // service related
    pub methods: SerializableSlab<ServiceMethod>,
    pub scripts: SerializableSlab<Script>,
    pub interceptors: SerializableSlab<Interceptor>,
}

impl ModelWasmSystem {
    pub fn schema_queries(&self) -> Vec<FieldDefinition> {
        self.queries
            .values
            .iter()
            .map(|(_, query)| query.field_definition(self))
            .collect()
    }

    pub fn schema_mutations(&self) -> Vec<FieldDefinition> {
        self.mutations
            .values
            .iter()
            .map(|(_, query)| query.field_definition(self))
            .collect()
    }

    pub fn schema_types(&self) -> Vec<TypeDefinition> {
        self.service_types
            .iter()
            .map(|typ| typ.1.type_definition(&self.service_types))
            .collect()
    }
}

impl SystemSerializer for ModelWasmSystem {
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
