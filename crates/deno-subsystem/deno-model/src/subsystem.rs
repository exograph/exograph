use async_graphql_parser::types::{FieldDefinition, TypeDefinition};

use core_plugin_interface::{
    core_model::{
        context_type::{ContextContainer, ContextType},
        mapped_arena::{MappedArena, SerializableSlab},
        type_normalization::{FieldDefinitionProvider, TypeDefinitionProvider},
    },
    error::ModelSerializationError,
    system_serializer::SystemSerializer,
};

use serde::{Deserialize, Serialize};

use super::module::Script;
use crate::{
    interceptor::Interceptor,
    module::ModuleMethod,
    operation::{DenoMutation, DenoQuery},
    types::ModuleType,
};

#[derive(Serialize, Deserialize, Debug)]
pub struct DenoSubsystem {
    pub contexts: MappedArena<ContextType>,
    pub module_types: SerializableSlab<ModuleType>,

    // query related
    pub queries: MappedArena<DenoQuery>,

    // mutation related
    pub mutations: MappedArena<DenoMutation>,

    // module related
    pub methods: SerializableSlab<ModuleMethod>,
    pub scripts: SerializableSlab<Script>,
    pub interceptors: SerializableSlab<Interceptor>,
}

impl DenoSubsystem {
    pub fn schema_queries(&self) -> Vec<FieldDefinition> {
        self.queries
            .iter()
            .map(|(_, query)| query.field_definition(self))
            .collect()
    }

    pub fn schema_mutations(&self) -> Vec<FieldDefinition> {
        self.mutations
            .iter()
            .map(|(_, query)| query.field_definition(self))
            .collect()
    }

    pub fn schema_types(&self) -> Vec<TypeDefinition> {
        self.module_types
            .iter()
            .map(|typ| typ.1.type_definition(&self.module_types))
            .collect()
    }
}

impl SystemSerializer for DenoSubsystem {
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

impl ContextContainer for DenoSubsystem {
    fn contexts(&self) -> &MappedArena<ContextType> {
        &self.contexts
    }
}
