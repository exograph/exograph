// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use super::module::Script;
use crate::{
    interceptor::Interceptor,
    module::ModuleMethod,
    operation::{WasmMutation, WasmQuery},
    types::ModuleType,
};
use async_graphql_parser::types::{FieldDefinition, TypeDefinition};
use core_model::{
    context_type::ContextType,
    mapped_arena::{MappedArena, SerializableSlab},
    type_normalization::{FieldDefinitionProvider, TypeDefinitionProvider},
};
use core_plugin_shared::system_serializer::{
    ModelSerializationError, SystemSerializer, postcard_deserialize, postcard_serialize,
};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct WasmSubsystem {
    pub contexts: MappedArena<ContextType>,
    pub module_types: SerializableSlab<ModuleType>,

    // query related
    pub queries: MappedArena<WasmQuery>,

    // mutation related
    pub mutations: MappedArena<WasmMutation>,

    // module related
    pub methods: SerializableSlab<ModuleMethod>,
    pub scripts: SerializableSlab<Script>,
    pub interceptors: SerializableSlab<Interceptor>,
}

impl WasmSubsystem {
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

impl SystemSerializer for WasmSubsystem {
    type Underlying = Self;

    fn serialize(&self) -> Result<Vec<u8>, ModelSerializationError> {
        postcard_serialize(self)
    }

    fn deserialize_reader(
        reader: impl std::io::Read,
    ) -> Result<Self::Underlying, ModelSerializationError> {
        postcard_deserialize(reader)
    }
}
