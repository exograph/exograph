// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::sync::Mutex;

use core_model::{
    access::AccessPredicateExpression,
    mapped_arena::{MappedArena, SerializableSlab, SerializableSlabIndex},
};
use core_model_builder::{builder::system_builder::BaseModelSystem, error::ModelBuildingError};

use postgres_core_model::{
    access::PrecheckAccessPrimitiveExpression,
    aggregate::AggregateType,
    predicate::PredicateParameterType,
    subsystem::PostgresCoreSubsystem,
    types::{EntityType, PostgresPrimitiveType},
    vector_distance::VectorDistanceType,
};

use postgres_core_model::access::DatabaseAccessPrimitiveExpression;

use exo_sql::Database;

use crate::{aggregate_type_builder, database_builder, predicate_builder, type_builder};

use crate::resolved_type::ResolvedTypeEnv;

pub fn build(resolved_env: &ResolvedTypeEnv) -> Result<SystemContextBuilding, ModelBuildingError> {
    let mut building = SystemContextBuilding {
        database: database_builder::build(resolved_env)?,
        ..SystemContextBuilding::default()
    };

    build_shallow(resolved_env, &mut building);
    build_expanded(resolved_env, &mut building)?;

    Ok(building)
}

/// Build shallow types, context, query parameters (order by and predicate)
fn build_shallow(resolved_env: &ResolvedTypeEnv, building: &mut SystemContextBuilding) {
    // The order of next three is unimportant, since each of them simply create a shallow type without referring to anything
    type_builder::build_shallow(resolved_env, building);

    aggregate_type_builder::build_shallow(resolved_env, building);

    predicate_builder::build_shallow(&resolved_env.resolved_types, building);
}

fn build_expanded(
    resolved_env: &ResolvedTypeEnv,
    building: &mut SystemContextBuilding,
) -> Result<(), ModelBuildingError> {
    // First fully build the types.
    type_builder::build_expanded(resolved_env, building)?;

    aggregate_type_builder::build_expanded(resolved_env, building)?;

    predicate_builder::build_expanded(resolved_env, building);

    Ok(())
}

#[derive(Debug, Default)]
pub struct SystemContextBuilding {
    pub primitive_types: MappedArena<PostgresPrimitiveType>,
    pub entity_types: MappedArena<EntityType>,
    pub aggregate_types: MappedArena<AggregateType>,
    pub predicate_types: MappedArena<PredicateParameterType>,
    pub vector_distance_types: MappedArena<VectorDistanceType>,

    pub database_access_expressions:
        Mutex<AccessExpressionsBuilding<DatabaseAccessPrimitiveExpression>>,
    pub precheck_access_expressions:
        Mutex<AccessExpressionsBuilding<PrecheckAccessPrimitiveExpression>>,

    pub database: Database,
}

impl SystemContextBuilding {
    pub fn into_core_subsystem(self, base_system: &BaseModelSystem) -> PostgresCoreSubsystem {
        PostgresCoreSubsystem {
            contexts: base_system.contexts.clone(),
            primitive_types: self.primitive_types.values(),
            entity_types: self.entity_types.values(),
            aggregate_types: self.aggregate_types.values(),
            predicate_types: self.predicate_types.values(),

            database: self.database,

            database_access_expressions: self
                .database_access_expressions
                .into_inner()
                .unwrap()
                .elems,
            precheck_expressions: self.precheck_access_expressions.into_inner().unwrap().elems,
        }
    }
}

/// Structure to keep track of access expressions arena and a special index for the oft-used restrictive access.
/// By keeping track of the restrictive access index, we avoid creating multiple indices for the same `False` expression.
#[derive(Debug)]
pub struct AccessExpressionsBuilding<T: Send + Sync> {
    elems: SerializableSlab<AccessPredicateExpression<T>>,
    restrictive_access_index: SerializableSlabIndex<AccessPredicateExpression<T>>,
    permissive_access_index: SerializableSlabIndex<AccessPredicateExpression<T>>,
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

    pub fn permissive_access_index(&self) -> SerializableSlabIndex<AccessPredicateExpression<T>> {
        self.permissive_access_index
    }
}

impl<T: Send + Sync> Default for AccessExpressionsBuilding<T> {
    fn default() -> Self {
        let mut elems = SerializableSlab::new();
        // Insert a default restrictive and permissive access expressions and keep around their indices
        let restrictive_access_index =
            elems.insert(AccessPredicateExpression::BooleanLiteral(false));
        let permissive_access_index = elems.insert(AccessPredicateExpression::BooleanLiteral(true));
        Self {
            elems,
            restrictive_access_index,
            permissive_access_index,
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
