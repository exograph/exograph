// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

//! Schema builder for RPC introspection.
//!
//! Builds an RpcSchema from PostgresRpcSubsystemWithRouter by iterating
//! through all collection queries and pk queries.

use core_model::types::{FieldType, OperationReturnType, TypeValidation};
use postgres_core_model::predicate::{PredicateParameter, PredicateParameterTypeKind};
use postgres_core_model::relation::PostgresRelation;
use postgres_core_model::types::{EntityType, PostgresFieldType, PostgresPrimitiveTypeKind};
use postgres_rpc_model::operation::{CollectionQuery, PkQuery};
use postgres_rpc_model::subsystem::PostgresRpcSubsystemWithRouter;
use rpc_introspection::schema::{
    RpcMethod, RpcObjectField, RpcObjectType, RpcParameter, RpcSchema, RpcTypeSchema,
};
use std::collections::HashSet;

/// Build an RpcSchema from a PostgresRpcSubsystemWithRouter.
pub fn build_rpc_schema(subsystem: &PostgresRpcSubsystemWithRouter) -> RpcSchema {
    let mut schema = RpcSchema::new();
    let mut added_types: HashSet<String> = HashSet::new();

    // Build methods from collection queries
    for (_, query) in subsystem.collection_queries.iter() {
        let method = build_collection_query_method(
            &query.name,
            query,
            subsystem,
            &mut schema,
            &mut added_types,
        );
        schema.add_method(method);
    }

    // Build methods from pk queries
    for (_, query) in subsystem.pk_queries.iter() {
        let method =
            build_pk_query_method(&query.name, query, subsystem, &mut schema, &mut added_types);
        schema.add_method(method);
    }

    schema
}

fn build_collection_query_method(
    name: &str,
    query: &CollectionQuery,
    subsystem: &PostgresRpcSubsystemWithRouter,
    schema: &mut RpcSchema,
    added_types: &mut HashSet<String>,
) -> RpcMethod {
    let entity_type = query
        .return_type
        .typ(&subsystem.core_subsystem.entity_types);
    let result_schema =
        build_return_type_schema(&query.return_type, subsystem, schema, added_types);

    let mut method = RpcMethod::new(name.to_string(), result_schema);

    // Auto-generated description for collection queries (like GraphQL)
    method = method.with_description(format!(
        "Get multiple `{}`s given the provided `where` filter, order by, limit, and offset",
        entity_type.name
    ));

    // Add "where" parameter (optional filter)
    let where_param = RpcParameter::new(
        "where",
        RpcTypeSchema::optional(build_predicate_param_schema(
            &query.parameters.predicate_param,
            subsystem,
            schema,
            added_types,
        )),
    )
    .with_description(format!("Filter conditions for {}", entity_type.plural_name));
    method = method.with_param(where_param);

    // Add "orderBy" parameter (optional ordering)
    let order_by_param = RpcParameter::new(
        "orderBy",
        RpcTypeSchema::optional(RpcTypeSchema::object(format!(
            "{}Ordering",
            entity_type.name
        ))),
    )
    .with_description("Ordering for the results");
    method = method.with_param(order_by_param);

    // Add "limit" parameter
    let limit_param = RpcParameter::new(
        "limit",
        RpcTypeSchema::optional(RpcTypeSchema::scalar("Int")),
    )
    .with_description("Maximum number of results to return");
    method = method.with_param(limit_param);

    // Add "offset" parameter
    let offset_param = RpcParameter::new(
        "offset",
        RpcTypeSchema::optional(RpcTypeSchema::scalar("Int")),
    )
    .with_description("Number of results to skip");
    method = method.with_param(offset_param);

    method
}

fn build_pk_query_method(
    name: &str,
    query: &PkQuery,
    subsystem: &PostgresRpcSubsystemWithRouter,
    schema: &mut RpcSchema,
    added_types: &mut HashSet<String>,
) -> RpcMethod {
    let entity_type = query
        .return_type
        .typ(&subsystem.core_subsystem.entity_types);
    let result_schema =
        build_return_type_schema(&query.return_type, subsystem, schema, added_types);

    let mut method = RpcMethod::new(name.to_string(), result_schema);

    // Auto-generated description for PK queries (like GraphQL)
    method = method.with_description(format!(
        "Get a single `{}` given primary key fields",
        entity_type.name
    ));

    // Add pk parameters (all required)
    for predicate_param in &query.parameters.predicate_params {
        let param_schema = build_pk_param_schema(predicate_param, subsystem);
        let param = RpcParameter::new(&predicate_param.name, param_schema)
            .with_description(format!("Primary key field: {}", predicate_param.name));
        method = method.with_param(param);
    }

    method
}

fn build_return_type_schema(
    return_type: &OperationReturnType<EntityType>,
    subsystem: &PostgresRpcSubsystemWithRouter,
    schema: &mut RpcSchema,
    added_types: &mut HashSet<String>,
) -> RpcTypeSchema {
    match return_type {
        OperationReturnType::Plain(base) => {
            let entity_type = &subsystem.core_subsystem.entity_types[base.associated_type_id];
            ensure_entity_type_added(entity_type, subsystem, schema, added_types);
            RpcTypeSchema::object(&entity_type.name)
        }
        OperationReturnType::Optional(inner) => RpcTypeSchema::optional(build_return_type_schema(
            inner,
            subsystem,
            schema,
            added_types,
        )),
        OperationReturnType::List(inner) => RpcTypeSchema::array(build_return_type_schema(
            inner,
            subsystem,
            schema,
            added_types,
        )),
    }
}

fn ensure_entity_type_added(
    entity_type: &EntityType,
    subsystem: &PostgresRpcSubsystemWithRouter,
    schema: &mut RpcSchema,
    added_types: &mut HashSet<String>,
) {
    if added_types.contains(&entity_type.name) {
        return;
    }
    added_types.insert(entity_type.name.clone());

    let mut obj_type = RpcObjectType::new(&entity_type.name);

    if let Some(doc) = &entity_type.doc_comments {
        obj_type = obj_type.with_description(doc);
    }

    for field in &entity_type.fields {
        // Only include scalar fields (not relations)
        if let PostgresRelation::Scalar { .. } = field.relation {
            let field_schema = build_field_type_schema(
                &field.typ,
                field.type_validation.as_ref(),
                subsystem,
                schema,
                added_types,
            );

            let mut obj_field = RpcObjectField::new(&field.name, field_schema);
            if let Some(doc) = &field.doc_comments {
                obj_field = obj_field.with_description(doc);
            }

            obj_type = obj_type.with_field(obj_field);
        }
    }

    schema.add_object_type(entity_type.name.clone(), obj_type);
}

fn build_field_type_schema(
    field_type: &FieldType<PostgresFieldType<EntityType>>,
    type_validation: Option<&TypeValidation>,
    subsystem: &PostgresRpcSubsystemWithRouter,
    schema: &mut RpcSchema,
    added_types: &mut HashSet<String>,
) -> RpcTypeSchema {
    match field_type {
        FieldType::Plain(postgres_field_type) => build_postgres_field_type_schema(
            postgres_field_type,
            type_validation,
            subsystem,
            schema,
            added_types,
        ),
        FieldType::Optional(inner) => RpcTypeSchema::optional(build_field_type_schema(
            inner,
            type_validation,
            subsystem,
            schema,
            added_types,
        )),
        FieldType::List(inner) => RpcTypeSchema::array(build_field_type_schema(
            inner,
            type_validation,
            subsystem,
            schema,
            added_types,
        )),
    }
}

fn build_postgres_field_type_schema(
    postgres_field_type: &PostgresFieldType<EntityType>,
    type_validation: Option<&TypeValidation>,
    subsystem: &PostgresRpcSubsystemWithRouter,
    schema: &mut RpcSchema,
    added_types: &mut HashSet<String>,
) -> RpcTypeSchema {
    let type_ref = postgres_field_type.type_id.to_type(
        &subsystem.core_subsystem.primitive_types,
        &subsystem.core_subsystem.entity_types,
    );

    match type_ref {
        postgres_core_model::types::PostgresType::Primitive(primitive) => {
            match &primitive.kind {
                PostgresPrimitiveTypeKind::Builtin => {
                    // Apply type validation if present
                    match type_validation {
                        Some(validation) => RpcTypeSchema::scalar_with_validation(
                            &primitive.name,
                            validation.clone(),
                        ),
                        None => RpcTypeSchema::scalar(&primitive.name),
                    }
                }
                PostgresPrimitiveTypeKind::Enum(values) => {
                    // Add enum type to schema if not already added
                    if !added_types.contains(&primitive.name) {
                        added_types.insert(primitive.name.clone());
                    }
                    RpcTypeSchema::enum_type(values.clone())
                }
            }
        }
        postgres_core_model::types::PostgresType::Composite(entity) => {
            ensure_entity_type_added(entity, subsystem, schema, added_types);
            RpcTypeSchema::object(&entity.name)
        }
    }
}

fn build_predicate_param_schema(
    param: &PredicateParameter,
    subsystem: &PostgresRpcSubsystemWithRouter,
    schema: &mut RpcSchema,
    added_types: &mut HashSet<String>,
) -> RpcTypeSchema {
    let param_type = &subsystem.core_subsystem.predicate_types[param.typ.innermost().type_id];

    // Build the filter type based on the parameter type kind
    match &param_type.kind {
        PredicateParameterTypeKind::ImplicitEqual => {
            // For implicit equals, use the field's scalar type
            RpcTypeSchema::scalar("String") // Simplified - actual type depends on field
        }
        PredicateParameterTypeKind::Operator(operators) => {
            // Build filter type with operators like eq, neq, lt, gt, etc.
            let filter_type_name = &param_type.name;
            if !added_types.contains(filter_type_name) {
                added_types.insert(filter_type_name.clone());
                let mut filter_obj = RpcObjectType::new(filter_type_name);
                for op_param in operators {
                    let op_schema = RpcTypeSchema::optional(RpcTypeSchema::scalar("String")); // Simplified
                    filter_obj =
                        filter_obj.with_field(RpcObjectField::new(&op_param.name, op_schema));
                }
                schema.add_object_type(filter_type_name.clone(), filter_obj);
            }
            RpcTypeSchema::object(filter_type_name)
        }
        PredicateParameterTypeKind::Composite {
            field_params,
            logical_op_params,
        } => {
            // Build composite filter type
            let filter_type_name = &param_type.name;
            if !added_types.contains(filter_type_name) {
                added_types.insert(filter_type_name.clone());
                let mut filter_obj = RpcObjectType::new(filter_type_name);

                // Add field predicates
                for field_param in field_params {
                    let field_schema = RpcTypeSchema::optional(build_predicate_param_schema(
                        field_param,
                        subsystem,
                        schema,
                        added_types,
                    ));
                    filter_obj =
                        filter_obj.with_field(RpcObjectField::new(&field_param.name, field_schema));
                }

                // Add logical operators (and, or, not)
                for logical_param in logical_op_params {
                    let logical_schema = RpcTypeSchema::optional(RpcTypeSchema::array(
                        RpcTypeSchema::object(filter_type_name),
                    ));
                    filter_obj = filter_obj
                        .with_field(RpcObjectField::new(&logical_param.name, logical_schema));
                }

                schema.add_object_type(filter_type_name.clone(), filter_obj);
            }
            RpcTypeSchema::object(filter_type_name)
        }
        PredicateParameterTypeKind::Reference(ref_params) => {
            // Reference to another entity's filter
            let ref_filter_name = &param_type.name;
            if !added_types.contains(ref_filter_name) {
                added_types.insert(ref_filter_name.clone());
                let mut filter_obj = RpcObjectType::new(ref_filter_name);
                for ref_param in ref_params {
                    let ref_schema = RpcTypeSchema::optional(build_predicate_param_schema(
                        ref_param,
                        subsystem,
                        schema,
                        added_types,
                    ));
                    filter_obj =
                        filter_obj.with_field(RpcObjectField::new(&ref_param.name, ref_schema));
                }
                schema.add_object_type(ref_filter_name.clone(), filter_obj);
            }
            RpcTypeSchema::object(ref_filter_name)
        }
        PredicateParameterTypeKind::Vector => {
            // Vector similarity search filter
            RpcTypeSchema::object("VectorFilter")
        }
    }
}

fn build_pk_param_schema(
    param: &PredicateParameter,
    subsystem: &PostgresRpcSubsystemWithRouter,
) -> RpcTypeSchema {
    // PK parameters use implicit equal semantics - they're required scalar values
    let param_type = &subsystem.core_subsystem.predicate_types[param.typ.innermost().type_id];

    // Try to determine the underlying scalar type from the parameter name
    // For now, we use a simple heuristic based on common patterns
    let type_name = match param.name.as_str() {
        "id" => "Int",
        _ => {
            // Default to the parameter type name, which might give us a hint
            match &param_type.underlying_type {
                Some(_) => "String", // Has underlying entity - it's a reference
                None => match param_type.name.as_str() {
                    name if name.ends_with("Filter") => "String",
                    _ => "String", // Default fallback
                },
            }
        }
    };

    RpcTypeSchema::scalar(type_name)
}

// Integration tests for this module are in integration-tests/rpc-introspection
// as they require a full PostgresRpcSubsystemWithRouter setup.
