// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use core_plugin_interface::{
    core_model::{
        access::AccessPredicateExpression,
        context_type::ContextType,
        mapped_arena::{MappedArena, SerializableSlab},
    },
    error::ModelSerializationError,
    system_serializer::SystemSerializer,
};
use exo_sql::Database;
use serde::{Deserialize, Serialize};

use crate::{
    access::{
        DatabaseAccessPrimitiveExpression, InputAccessPrimitiveExpression,
        PrecheckAccessPrimitiveExpression,
    },
    aggregate::AggregateType,
    types::{EntityType, PostgresPrimitiveType},
};

#[derive(Serialize, Deserialize, Debug)]
pub struct PostgresCoreSubsystem {
    pub contexts: MappedArena<ContextType>,
    pub primitive_types: SerializableSlab<PostgresPrimitiveType>,
    pub entity_types: SerializableSlab<EntityType>,

    pub aggregate_types: SerializableSlab<AggregateType>,

    pub input_access_expressions:
        SerializableSlab<AccessPredicateExpression<InputAccessPrimitiveExpression>>,
    pub database_access_expressions:
        SerializableSlab<AccessPredicateExpression<DatabaseAccessPrimitiveExpression>>,
    pub precheck_expressions:
        SerializableSlab<AccessPredicateExpression<PrecheckAccessPrimitiveExpression>>,

    pub database: Database,
}

impl SystemSerializer for PostgresCoreSubsystem {
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

impl Default for PostgresCoreSubsystem {
    fn default() -> Self {
        Self {
            contexts: MappedArena::default(),
            primitive_types: SerializableSlab::new(),
            entity_types: SerializableSlab::new(),
            aggregate_types: SerializableSlab::new(),

            input_access_expressions: SerializableSlab::new(),
            database_access_expressions: SerializableSlab::new(),
            precheck_expressions: SerializableSlab::new(),

            database: Database::default(),
        }
    }
}
