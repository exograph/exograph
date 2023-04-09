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
    module::ModuleMethod,
    operation::{ModuleMutation, ModuleQuery},
    types::ModuleType,
};

use super::module::Script;

#[derive(Serialize, Deserialize, Debug)]
pub struct ModuleSubsystem {
    pub contexts: MappedArena<ContextType>,
    pub module_types: SerializableSlab<ModuleType>,

    // query related
    pub queries: MappedArena<ModuleQuery>,

    // mutation related
    pub mutations: MappedArena<ModuleMutation>,

    // module related
    pub methods: SerializableSlab<ModuleMethod>,
    pub scripts: SerializableSlab<Script>,
    pub interceptors: SerializableSlab<Interceptor>,
}

impl ModuleSubsystem {
    pub fn schema_queries(&self) -> Vec<Positioned<FieldDefinition>> {
        self.queries
            .iter()
            .map(|query| default_positioned(query.1.field_definition(self)))
            .collect()
    }

    pub fn schema_mutations(&self) -> Vec<Positioned<FieldDefinition>> {
        self.mutations
            .iter()
            .map(|query| default_positioned(query.1.field_definition(self)))
            .collect()
    }

    pub fn schema_types(&self) -> Vec<TypeDefinition> {
        self.module_types
            .iter()
            .map(|typ| typ.1.type_definition(&self.module_types))
            .collect()
    }
}
