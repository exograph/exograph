// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::{collections::HashMap, sync::Arc};

use core_model::mapped_arena::{MappedArena, SerializableSlabIndex};
use core_model_builder::error::ModelBuildingError;

use postgres_graphql_model::{
    mutation::PostgresMutation,
    query::{AggregateQuery, CollectionQuery, UniqueQuery},
    subsystem::PostgresGraphQLSubsystem,
    types::MutationType,
};

use postgres_core_model::types::EntityType;

use super::{mutation_builder, query_builder, type_builder};
use postgres_core_builder::resolved_type::ResolvedTypeEnv;

pub fn build(
    resolved_env: &ResolvedTypeEnv,
    core_subsystem_building: Arc<postgres_core_builder::SystemContextBuilding>,
) -> Result<Option<PostgresGraphQLSubsystem>, ModelBuildingError> {
    let mut building = SystemContextBuilding {
        core_subsystem: core_subsystem_building.clone(),
        ..SystemContextBuilding::default()
    };

    let system = {
        build_shallow(resolved_env, &mut building);
        build_expanded(resolved_env, &mut building)?;

        PostgresGraphQLSubsystem {
            pk_queries: building.pk_queries,
            collection_queries: building.collection_queries,
            aggregate_queries: building.aggregate_queries,
            unique_queries: building.unique_queries,
            mutation_types: building.mutation_types.values(),
            mutations: building.mutations,

            pk_queries_map: building.pk_queries_map,
            collection_queries_map: building.collection_queries_map,
            aggregate_queries_map: building.aggregate_queries_map,

            ..Default::default()
        }
    };

    Ok({
        if system.pk_queries.is_empty()
            && system.collection_queries.is_empty()
            && system.aggregate_queries.is_empty()
            && system.mutations.is_empty()
        {
            None
        } else {
            Some(system)
        }
    })
}

/// Build shallow types, context, query parameters (order by and predicate)
fn build_shallow(resolved_env: &ResolvedTypeEnv, building: &mut SystemContextBuilding) {
    // The shallow builders need POSTGRES types built in core (the order of the next two is unimportant)
    // Specifically, the OperationReturn type in Query and Mutation looks for the id for the return type, so requires
    // type_builder::build_shallow to have run
    query_builder::build_shallow(&resolved_env.resolved_types, building);
    mutation_builder::build_shallow(&resolved_env.resolved_types, building);
}

fn build_expanded(
    resolved_env: &ResolvedTypeEnv,
    building: &mut SystemContextBuilding,
) -> Result<(), ModelBuildingError> {
    // First fully build the types.
    type_builder::build_expanded(resolved_env, building)?;

    // Finally expand queries, mutations, and module methods
    query_builder::build_expanded(resolved_env, building);
    mutation_builder::build_expanded(building)?;

    Ok(())
}

#[derive(Debug, Default)]
pub struct SystemContextBuilding {
    pub pk_queries: MappedArena<UniqueQuery>,
    pub collection_queries: MappedArena<CollectionQuery>,
    pub aggregate_queries: MappedArena<AggregateQuery>,
    pub unique_queries: MappedArena<UniqueQuery>,

    pub pk_queries_map:
        HashMap<SerializableSlabIndex<EntityType>, SerializableSlabIndex<UniqueQuery>>,
    pub collection_queries_map:
        HashMap<SerializableSlabIndex<EntityType>, SerializableSlabIndex<CollectionQuery>>,
    pub aggregate_queries_map:
        HashMap<SerializableSlabIndex<EntityType>, SerializableSlabIndex<AggregateQuery>>,

    pub mutation_types: MappedArena<MutationType>,
    pub mutations: MappedArena<PostgresMutation>,

    pub core_subsystem: Arc<postgres_core_builder::SystemContextBuilding>,
}

impl SystemContextBuilding {
    pub fn get_entity_type_id(&self, name: &str) -> Option<SerializableSlabIndex<EntityType>> {
        self.core_subsystem.entity_types.get_id(name)
    }
}
