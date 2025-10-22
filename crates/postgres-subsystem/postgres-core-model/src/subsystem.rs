// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use core_model::{
    access::AccessPredicateExpression,
    context_type::{ContextContainer, ContextType},
    mapped_arena::{MappedArena, SerializableSlab},
};
use core_plugin_shared::{error::ModelSerializationError, system_serializer::SystemSerializer};
use exo_sql::Database;
use serde::{Deserialize, Serialize};

use crate::{
    access::{DatabaseAccessPrimitiveExpression, PrecheckAccessPrimitiveExpression},
    aggregate::AggregateType,
    types::{EntityType, PostgresPrimitiveType},
};

#[derive(Serialize, Deserialize, Debug)]
pub struct PostgresCoreSubsystem {
    pub contexts: MappedArena<ContextType>,
    pub primitive_types: SerializableSlab<PostgresPrimitiveType>,
    pub entity_types: SerializableSlab<EntityType>,

    pub aggregate_types: SerializableSlab<AggregateType>,

    pub database_access_expressions:
        SerializableSlab<AccessPredicateExpression<DatabaseAccessPrimitiveExpression>>,
    pub precheck_expressions:
        SerializableSlab<AccessPredicateExpression<PrecheckAccessPrimitiveExpression>>,

    pub database: Database,
}

impl SystemSerializer for PostgresCoreSubsystem {
    type Underlying = Self;

    fn serialize(&self) -> Result<Vec<u8>, ModelSerializationError> {
        bincode::serde::encode_to_vec(self, bincode::config::standard())
            .map_err(ModelSerializationError::Serialize)
    }

    fn deserialize_reader(
        mut reader: impl std::io::Read,
    ) -> Result<Self::Underlying, ModelSerializationError> {
        bincode::serde::decode_from_std_read(&mut reader, bincode::config::standard())
            .map_err(ModelSerializationError::Deserialize)
    }
}

impl Default for PostgresCoreSubsystem {
    fn default() -> Self {
        Self {
            contexts: MappedArena::default(),
            primitive_types: SerializableSlab::new(),
            entity_types: SerializableSlab::new(),
            aggregate_types: SerializableSlab::new(),

            database_access_expressions: SerializableSlab::new(),
            precheck_expressions: SerializableSlab::new(),

            database: Database::default(),
        }
    }
}

impl ContextContainer for PostgresCoreSubsystem {
    fn contexts(&self) -> &MappedArena<ContextType> {
        &self.contexts
    }
}
