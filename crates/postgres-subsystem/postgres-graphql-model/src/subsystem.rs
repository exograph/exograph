// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::{collections::HashMap, sync::Arc, vec};

use async_graphql_parser::types::{FieldDefinition, TypeDefinition};

use super::{
    mutation::PostgresMutation, order::OrderByParameterType, predicate::PredicateParameterType,
    query::PkQuery,
};
use crate::{
    query::{AggregateQuery, CollectionQuery, UniqueQuery},
    types::MutationType,
};
use core_plugin_interface::{
    core_model::{
        access::AccessPredicateExpression,
        context_type::{ContextContainer, ContextType},
        mapped_arena::{MappedArena, SerializableSlab, SerializableSlabIndex},
        type_normalization::{FieldDefinitionProvider, TypeDefinitionProvider},
    },
    error::ModelSerializationError,
    system_serializer::SystemSerializer,
};

use postgres_core_model::access::{
    DatabaseAccessPrimitiveExpression, InputAccessPrimitiveExpression,
};
use postgres_core_model::{
    aggregate::AggregateType,
    types::{EntityType, PostgresPrimitiveType},
};

use exo_sql::Database;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct PostgresGraphQLSubsystem {
    pub contexts: MappedArena<ContextType>,
    pub primitive_types: SerializableSlab<PostgresPrimitiveType>,
    pub entity_types: SerializableSlab<EntityType>,

    pub aggregate_types: SerializableSlab<AggregateType>,

    // query related
    pub order_by_types: SerializableSlab<OrderByParameterType>,
    pub predicate_types: SerializableSlab<PredicateParameterType>,

    pub pk_queries: MappedArena<PkQuery>,
    pub collection_queries: MappedArena<CollectionQuery>,
    pub aggregate_queries: MappedArena<AggregateQuery>,
    pub unique_queries: MappedArena<UniqueQuery>,

    pub pk_queries_map: HashMap<SerializableSlabIndex<EntityType>, SerializableSlabIndex<PkQuery>>,
    pub collection_queries_map:
        HashMap<SerializableSlabIndex<EntityType>, SerializableSlabIndex<CollectionQuery>>,
    pub aggregate_queries_map:
        HashMap<SerializableSlabIndex<EntityType>, SerializableSlabIndex<AggregateQuery>>,

    // mutation related
    pub mutation_types: SerializableSlab<MutationType>, // create, update, delete input types such as `PersonUpdateInput`
    pub mutations: MappedArena<PostgresMutation>,

    pub input_access_expressions:
        SerializableSlab<AccessPredicateExpression<InputAccessPrimitiveExpression>>,
    pub database_access_expressions:
        SerializableSlab<AccessPredicateExpression<DatabaseAccessPrimitiveExpression>>,

    #[serde(skip)]
    pub database: Arc<Database>,
}

impl PostgresGraphQLSubsystem {
    pub fn schema_queries(&self) -> Vec<FieldDefinition> {
        let pk_queries_defn = self
            .pk_queries
            .iter()
            .map(|(_, query)| query.field_definition(self));

        let collection_queries_defn = self
            .collection_queries
            .iter()
            .map(|(_, query)| query.field_definition(self));

        let aggregate_queries_defn = self
            .aggregate_queries
            .iter()
            .map(|query| query.1.field_definition(self));

        let unique_queries_defn = self
            .unique_queries
            .iter()
            .map(|(_, query)| query.field_definition(self));

        pk_queries_defn
            .chain(collection_queries_defn)
            .chain(aggregate_queries_defn)
            .chain(unique_queries_defn)
            .collect()
    }

    pub fn schema_mutations(&self) -> Vec<FieldDefinition> {
        self.mutations
            .iter()
            .map(|(_, mutation)| mutation.field_definition(self))
            .collect()
    }

    pub fn schema_types(&self) -> Vec<TypeDefinition> {
        let mut all_type_definitions = vec![];

        self.primitive_types
            .iter()
            .for_each(|typ| all_type_definitions.push(typ.1.type_definition(self)));

        self.entity_types
            .iter()
            .for_each(|typ| all_type_definitions.push(typ.1.type_definition(self)));

        self.aggregate_types
            .iter()
            .for_each(|typ| all_type_definitions.push(typ.1.type_definition(self)));

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

    pub fn get_pk_query(&self, entity_type_id: SerializableSlabIndex<EntityType>) -> &PkQuery {
        let pk_query_index = self.pk_queries_map[&entity_type_id];
        &self.pk_queries[pk_query_index]
    }

    pub fn get_collection_query(
        &self,
        entity_type_id: SerializableSlabIndex<EntityType>,
    ) -> &CollectionQuery {
        let collection_query_index = self.collection_queries_map[&entity_type_id];
        &self.collection_queries[collection_query_index]
    }

    pub fn get_aggregate_query(
        &self,
        entity_type_id: SerializableSlabIndex<EntityType>,
    ) -> &AggregateQuery {
        let aggregate_query_index = self.aggregate_queries_map[&entity_type_id];
        &self.aggregate_queries[aggregate_query_index]
    }
}

impl Default for PostgresGraphQLSubsystem {
    fn default() -> Self {
        Self {
            contexts: MappedArena::default(),
            primitive_types: SerializableSlab::new(),
            entity_types: SerializableSlab::new(),
            aggregate_types: SerializableSlab::new(),
            order_by_types: SerializableSlab::new(),
            predicate_types: SerializableSlab::new(),
            pk_queries: MappedArena::default(),
            collection_queries: MappedArena::default(),
            aggregate_queries: MappedArena::default(),
            unique_queries: MappedArena::default(),
            mutation_types: SerializableSlab::new(),
            mutations: MappedArena::default(),

            pk_queries_map: HashMap::new(),
            collection_queries_map: HashMap::new(),
            aggregate_queries_map: HashMap::new(),

            input_access_expressions: SerializableSlab::new(),
            database_access_expressions: SerializableSlab::new(),

            database: Arc::new(Database::default()),
        }
    }
}

impl SystemSerializer for PostgresGraphQLSubsystem {
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

impl ContextContainer for PostgresGraphQLSubsystem {
    fn contexts(&self) -> &MappedArena<ContextType> {
        &self.contexts
    }
}
