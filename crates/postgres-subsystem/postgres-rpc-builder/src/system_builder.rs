// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::sync::Arc;

use core_model::access::AccessPredicateExpression;
use core_model::mapped_arena::MappedArena;
use core_model_builder::error::ModelBuildingError;

use postgres_core_builder::order_by_builder::new_root_param;
use postgres_core_builder::resolved_type::{ResolvedType, ResolvedTypeEnv};
use postgres_core_model::doc_comments;
use postgres_core_model::predicate::PredicateParameter;
use postgres_core_model::types::{EntityRepresentation, EntityType};
use postgres_rpc_model::operation::{
    CollectionDelete, CollectionDeleteParameters, CollectionQuery, CollectionQueryParameters,
    PkDelete, PkQuery, PostgresOperation, ScalarParam, UniqueDelete, UniqueQuery,
};
use postgres_rpc_model::subsystem::PostgresRpcSubsystem;

use crate::helper::{
    build_filter_predicate_param, build_pk_predicate_params, build_unique_predicate_params,
    list_return_type, optional_return_type,
};
use crate::naming;

pub fn build(
    resolved_env: &ResolvedTypeEnv<'_>,
    core_subsystem_building: Arc<postgres_core_builder::SystemContextBuilding>,
) -> Result<Option<PostgresRpcSubsystem>, ModelBuildingError> {
    let mut collection_queries = MappedArena::default();
    let mut pk_queries = MappedArena::default();
    let mut unique_queries = MappedArena::default();
    let mut pk_deletes = MappedArena::default();
    let mut unique_deletes = MappedArena::default();
    let mut collection_deletes = MappedArena::default();

    for typ in resolved_env.resolved_types.iter() {
        if let ResolvedType::Composite(composite) = typ.1 {
            if composite.representation == EntityRepresentation::Json {
                continue;
            }

            let entity_type_id = core_subsystem_building
                .entity_types
                .get_id(&composite.name)
                .ok_or(ModelBuildingError::Generic(format!(
                    "Entity type not found: {}",
                    composite.name
                )))?;

            let entity_type = &core_subsystem_building.entity_types[entity_type_id];

            build_queries(
                composite,
                entity_type,
                entity_type_id,
                &core_subsystem_building,
                &mut collection_queries,
                &mut pk_queries,
                &mut unique_queries,
            )?;

            build_deletes(
                composite,
                entity_type,
                entity_type_id,
                &core_subsystem_building,
                &mut pk_deletes,
                &mut unique_deletes,
                &mut collection_deletes,
            )?;
        }
    }

    if collection_queries.is_empty()
        && pk_queries.is_empty()
        && unique_queries.is_empty()
        && pk_deletes.is_empty()
        && unique_deletes.is_empty()
        && collection_deletes.is_empty()
    {
        return Ok(None);
    }

    Ok(Some(PostgresRpcSubsystem {
        pk_queries,
        unique_queries,
        collection_queries,
        pk_deletes,
        unique_deletes,
        collection_deletes,
        core_subsystem: Default::default(),
    }))
}

fn build_queries(
    composite: &postgres_core_builder::resolved_type::ResolvedCompositeType,
    entity_type: &EntityType,
    entity_type_id: core_model::mapped_arena::SerializableSlabIndex<EntityType>,
    core_subsystem_building: &postgres_core_builder::SystemContextBuilding,
    collection_queries: &mut MappedArena<CollectionQuery>,
    pk_queries: &mut MappedArena<PkQuery>,
    unique_queries: &mut MappedArena<UniqueQuery>,
) -> Result<(), ModelBuildingError> {
    // Build collection query (get_todos) - returns multiple items
    let collection_method = naming::get_collection(&composite.plural_name);

    let predicate_param = build_filter_predicate_param(&composite.name, core_subsystem_building)?;

    let order_by_param = new_root_param(
        &composite.name,
        false,
        &core_subsystem_building.order_by_types,
    );

    let limit_param = ScalarParam {
        name: postgres_core_model::limit_offset::LIMIT_PARAM_NAME.to_string(),
        description: postgres_core_model::limit_offset::LIMIT_PARAM_DESCRIPTION.to_string(),
        type_name: core_model::primitive_type::IntType::NAME.to_string(),
    };

    let offset_param = ScalarParam {
        name: postgres_core_model::limit_offset::OFFSET_PARAM_NAME.to_string(),
        description: postgres_core_model::limit_offset::OFFSET_PARAM_DESCRIPTION.to_string(),
        type_name: core_model::primitive_type::IntType::NAME.to_string(),
    };

    let collection_query = CollectionQuery {
        name: collection_method.clone(),
        parameters: CollectionQueryParameters {
            predicate_param,
            order_by_param,
            limit_param,
            offset_param,
        },
        return_type: list_return_type(entity_type_id, &composite.name),
        doc_comments: Some(doc_comments::collection_query_description(&composite.name)),
    };

    collection_queries.add(&collection_method, collection_query);

    // Build PK query (get_todo)
    let get_method = naming::get_single(&composite.name);
    build_pk_operation(
        composite,
        entity_type,
        entity_type_id,
        core_subsystem_building,
        &get_method,
        &doc_comments::pk_query_description(&composite.name),
        pk_queries,
    );

    // Build unique queries (get_todo_by_username, etc.)
    build_unique_operations(
        composite,
        entity_type,
        entity_type_id,
        core_subsystem_building,
        |constraint_name| naming::get_single_by_unique(&composite.name, constraint_name),
        |constraint_name| doc_comments::unique_query_description(&composite.name, constraint_name),
        unique_queries,
    )?;

    Ok(())
}

fn build_deletes(
    composite: &postgres_core_builder::resolved_type::ResolvedCompositeType,
    entity_type: &EntityType,
    entity_type_id: core_model::mapped_arena::SerializableSlabIndex<EntityType>,
    core_subsystem_building: &postgres_core_builder::SystemContextBuilding,
    pk_deletes: &mut MappedArena<PkDelete>,
    unique_deletes: &mut MappedArena<UniqueDelete>,
    collection_deletes: &mut MappedArena<CollectionDelete>,
) -> Result<(), ModelBuildingError> {
    // Skip if delete access is explicitly false
    let skip_delete = matches!(
        core_subsystem_building
            .database_access_expressions
            .lock()
            .unwrap()[entity_type.access.delete],
        AccessPredicateExpression::BooleanLiteral(false)
    );

    if skip_delete {
        return Ok(());
    }

    let delete_method = naming::delete_single(&composite.name);
    let delete_collection_method = naming::delete_collection(&composite.plural_name);

    // Build PK delete (delete_todo)
    build_pk_operation(
        composite,
        entity_type,
        entity_type_id,
        core_subsystem_building,
        &delete_method,
        &doc_comments::pk_delete_description(&composite.name),
        pk_deletes,
    );

    // Build unique deletes (delete_todo_by_username, etc.)
    build_unique_operations(
        composite,
        entity_type,
        entity_type_id,
        core_subsystem_building,
        |constraint_name| naming::delete_single_by_unique(&composite.name, constraint_name),
        |constraint_name| doc_comments::unique_delete_description(&composite.name, constraint_name),
        unique_deletes,
    )?;

    // Build collection delete (delete_todos) - returns List<PK>
    let predicate_param = build_filter_predicate_param(&composite.name, core_subsystem_building)?;

    let collection_delete = CollectionDelete {
        name: delete_collection_method.clone(),
        parameters: CollectionDeleteParameters { predicate_param },
        return_type: list_return_type(entity_type_id, &composite.name),
        doc_comments: Some(doc_comments::collection_delete_description(&composite.name)),
    };

    collection_deletes.add(&delete_collection_method, collection_delete);

    Ok(())
}

fn build_pk_operation<P>(
    composite: &postgres_core_builder::resolved_type::ResolvedCompositeType,
    entity_type: &EntityType,
    entity_type_id: core_model::mapped_arena::SerializableSlabIndex<EntityType>,
    core_subsystem_building: &postgres_core_builder::SystemContextBuilding,
    method_name: &str,
    description: &str,
    ops: &mut MappedArena<PostgresOperation<P>>,
) where
    P: From<Vec<PredicateParameter>>,
{
    let pk_fields = entity_type.pk_fields();

    if let Some(pk_params) = build_pk_predicate_params(&pk_fields, core_subsystem_building) {
        let pk_op = PostgresOperation {
            name: method_name.to_string(),
            parameters: P::from(pk_params),
            return_type: optional_return_type(entity_type_id, &composite.name),
            doc_comments: Some(description.to_string()),
        };

        ops.add(method_name, pk_op);
    }
}

/// Build unique constraint operations for an entity.
fn build_unique_operations<P>(
    composite: &postgres_core_builder::resolved_type::ResolvedCompositeType,
    entity_type: &EntityType,
    entity_type_id: core_model::mapped_arena::SerializableSlabIndex<EntityType>,
    core_subsystem_building: &postgres_core_builder::SystemContextBuilding,
    method_name_fn: impl Fn(&str) -> String,
    description_fn: impl Fn(&str) -> String,
    ops: &mut MappedArena<PostgresOperation<P>>,
) -> Result<(), ModelBuildingError>
where
    P: From<Vec<PredicateParameter>>,
{
    for (constraint_name, constraint_fields) in composite.unique_constraints() {
        let predicate_params = build_unique_predicate_params(
            &constraint_fields,
            entity_type,
            core_subsystem_building,
        )?;

        let method_name = method_name_fn(&constraint_name);
        let description = description_fn(&constraint_name);
        let unique_op = PostgresOperation {
            name: method_name.clone(),
            parameters: P::from(predicate_params),
            return_type: optional_return_type(entity_type_id, &composite.name),
            doc_comments: Some(description),
        };

        ops.add(&method_name, unique_op);
    }

    Ok(())
}
