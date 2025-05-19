// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use core_model_builder::error::ModelBuildingError;
use postgres_core_builder::resolved_type::{ResolvedCompositeType, ResolvedType, ResolvedTypeEnv};
use postgres_core_model::types::EntityRepresentation;

use super::naming::ToPostgresQueryName;
use super::system_builder::SystemContextBuilding;

pub(super) fn build_expanded(
    resolved_env: &ResolvedTypeEnv,
    building: &mut SystemContextBuilding,
) -> Result<(), ModelBuildingError> {
    for (_, resolved_type) in resolved_env.resolved_types.iter() {
        if let ResolvedType::Composite(c) = &resolved_type {
            if c.representation == EntityRepresentation::Json {
                continue;
            }
            expand_query_mutation_map(c, building);
        }
    }

    Ok(())
}

fn expand_query_mutation_map(
    resolved_type: &ResolvedCompositeType,
    building: &mut SystemContextBuilding,
) {
    let existing_type_id = building.get_entity_type_id(&resolved_type.name).unwrap();

    if let Some(pk_query) = building.pk_queries.get_id(&resolved_type.pk_query()) {
        building.pk_queries_map.insert(existing_type_id, pk_query);
    }

    if let Some(collection_query) = building
        .collection_queries
        .get_id(&resolved_type.collection_query())
    {
        building
            .collection_queries_map
            .insert(existing_type_id, collection_query);
    }

    if let Some(aggregate_query) = building
        .aggregate_queries
        .get_id(&resolved_type.aggregate_query())
    {
        building
            .aggregate_queries_map
            .insert(existing_type_id, aggregate_query);
    }

    // building
    //     .collection_queries_map
    //     .insert(existing_type_id, collection_query);
    // building
    //     .aggregate_queries_map
    //     .insert(existing_type_id, aggregate_query);
}
