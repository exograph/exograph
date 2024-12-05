// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::{cell::RefCell, collections::HashMap, sync::Arc};

use core_plugin_interface::{
    core_model::{
        access::AccessPredicateExpression,
        mapped_arena::{MappedArena, SerializableSlab, SerializableSlabIndex},
    },
    core_model_builder::{builder::system_builder::BaseModelSystem, error::ModelBuildingError},
};

use postgres_graphql_model::{
    mutation::PostgresMutation,
    order::OrderByParameterType,
    predicate::PredicateParameterType,
    query::{AggregateQuery, CollectionQuery, PkQuery, UniqueQuery},
    subsystem::PostgresGraphQLSubsystem,
    types::MutationType,
};

use postgres_core_model::{
    aggregate::AggregateType,
    types::{EntityType, PostgresPrimitiveType},
    vector_distance::VectorDistanceType,
};

use postgres_core_model::access::{
    DatabaseAccessPrimitiveExpression, InputAccessPrimitiveExpression,
};

use exo_sql::Database;

use crate::aggregate_type_builder;

use super::{
    mutation_builder, order_by_type_builder, predicate_builder, query_builder, type_builder,
};
use postgres_core_builder::resolved_type::ResolvedTypeEnv;

pub fn build(
    resolved_env: &ResolvedTypeEnv,
    base_system: &BaseModelSystem,
    database: Arc<Database>,
) -> Result<Option<PostgresGraphQLSubsystem>, ModelBuildingError> {
    let mut building = SystemContextBuilding {
        database,
        ..SystemContextBuilding::default()
    };

    let system = {
        build_shallow(resolved_env, &mut building);
        build_expanded(resolved_env, &mut building)?;

        PostgresGraphQLSubsystem {
            contexts: base_system.contexts.clone(),
            primitive_types: building.primitive_types.values(),
            entity_types: building.entity_types.values(),
            aggregate_types: building.aggregate_types.values(),

            order_by_types: building.order_by_types.values(),
            predicate_types: building.predicate_types.values(),
            pk_queries: building.pk_queries,
            collection_queries: building.collection_queries,
            aggregate_queries: building.aggregate_queries,
            unique_queries: building.unique_queries,
            database: building.database,
            mutation_types: building.mutation_types.values(),
            mutations: building.mutations,

            pk_queries_map: building.pk_queries_map,
            collection_queries_map: building.collection_queries_map,
            aggregate_queries_map: building.aggregate_queries_map,

            input_access_expressions: building.input_access_expressions.into_inner().elems,
            database_access_expressions: building.database_access_expressions.into_inner().elems,
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
    // The order of next three is unimportant, since each of them simply create a shallow type without referring to anything
    type_builder::build_shallow(resolved_env, building);

    order_by_type_builder::build_shallow(resolved_env, building);

    predicate_builder::build_shallow(&resolved_env.resolved_types, building);

    aggregate_type_builder::build_shallow(resolved_env, building);

    // The next two shallow builders need POSTGRES types build above (the order of the next two is unimportant)
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

    // Which is then used to expand query and query parameters (the order is unimportant) but must be executed
    // after running type_builder::build_expanded (since they depend on expanded PostgresTypes (note the next ones do not access resolved_types))
    order_by_type_builder::build_expanded(resolved_env, building);
    predicate_builder::build_expanded(resolved_env, building);
    aggregate_type_builder::build_expanded(resolved_env, building)?;

    // Finally expand queries, mutations, and module methods
    query_builder::build_expanded(resolved_env, building);
    mutation_builder::build_expanded(building)?;

    Ok(())
}

#[derive(Debug, Default)]
pub struct SystemContextBuilding {
    pub primitive_types: MappedArena<PostgresPrimitiveType>,
    pub entity_types: MappedArena<EntityType>,

    pub aggregate_types: MappedArena<AggregateType>,
    pub vector_distance_types: MappedArena<VectorDistanceType>,
    pub order_by_types: MappedArena<OrderByParameterType>,
    pub predicate_types: MappedArena<PredicateParameterType>,

    pub pk_queries: MappedArena<PkQuery>,
    pub collection_queries: MappedArena<CollectionQuery>,
    pub aggregate_queries: MappedArena<AggregateQuery>,
    pub unique_queries: MappedArena<UniqueQuery>,

    pub pk_queries_map: HashMap<SerializableSlabIndex<EntityType>, SerializableSlabIndex<PkQuery>>,
    pub collection_queries_map:
        HashMap<SerializableSlabIndex<EntityType>, SerializableSlabIndex<CollectionQuery>>,
    pub aggregate_queries_map:
        HashMap<SerializableSlabIndex<EntityType>, SerializableSlabIndex<AggregateQuery>>,

    pub mutation_types: MappedArena<MutationType>,
    pub mutations: MappedArena<PostgresMutation>,

    pub input_access_expressions:
        RefCell<AccessExpressionsBuilding<InputAccessPrimitiveExpression>>,
    pub database_access_expressions:
        RefCell<AccessExpressionsBuilding<DatabaseAccessPrimitiveExpression>>,

    pub database: Arc<Database>,
}

/// Structure to keep track of access expressions arena and a special index for the oft-used restrictive access.
/// By keeping track of the restrictive access index, we avoid creating multiple indices for the same `False` expression.
#[derive(Debug)]
pub struct AccessExpressionsBuilding<T: Send + Sync> {
    elems: SerializableSlab<AccessPredicateExpression<T>>,
    restrictive_access_index: SerializableSlabIndex<AccessPredicateExpression<T>>,
}

impl<T: Send + Sync> AccessExpressionsBuilding<T> {
    pub fn insert(
        &mut self,
        elem: AccessPredicateExpression<T>,
    ) -> SerializableSlabIndex<AccessPredicateExpression<T>> {
        self.elems.insert(elem)
    }

    pub fn restricted_access_index(&self) -> SerializableSlabIndex<AccessPredicateExpression<T>> {
        self.restrictive_access_index
    }
}

impl<T: Send + Sync> Default for AccessExpressionsBuilding<T> {
    fn default() -> Self {
        let mut elems = SerializableSlab::new();
        // Insert a default restrictive access expression and keep around its index
        let restrictive_access_index =
            elems.insert(AccessPredicateExpression::BooleanLiteral(false));
        Self {
            elems,
            restrictive_access_index,
        }
    }
}

impl<T: Send + Sync> core::ops::Index<SerializableSlabIndex<AccessPredicateExpression<T>>>
    for AccessExpressionsBuilding<T>
{
    type Output = AccessPredicateExpression<T>;

    fn index(&self, index: SerializableSlabIndex<AccessPredicateExpression<T>>) -> &Self::Output {
        &self.elems[index]
    }
}

impl SystemContextBuilding {
    pub fn get_entity_type_id(&self, name: &str) -> Option<SerializableSlabIndex<EntityType>> {
        self.entity_types.get_id(name)
    }
}
