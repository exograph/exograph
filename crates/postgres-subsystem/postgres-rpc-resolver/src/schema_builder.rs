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
use postgres_core_model::order::{
    OrderByParameter, OrderByParameterTypeKind, PRIMITIVE_ORDERING_OPTIONS,
};
use postgres_core_model::predicate::{PredicateParameter, PredicateParameterTypeKind};
use postgres_core_model::relation::PostgresRelation;
use postgres_core_model::types::{EntityType, PostgresFieldType, PostgresPrimitiveTypeKind};
use postgres_rpc_model::operation::{CollectionQuery, PkQuery};
use postgres_rpc_model::subsystem::PostgresRpcSubsystemWithRouter;
use rpc_introspection::schema::{
    RpcMethod, RpcObjectField, RpcObjectType, RpcParameter, RpcSchema, RpcTypeSchema,
};
use std::collections::HashSet;

/// Name for the primitive ordering enum type
const ORDERING_ENUM_NAME: &str = "Ordering";

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
    let order_by_schema = build_order_by_param_schema(
        &query.parameters.order_by_param,
        subsystem,
        schema,
        added_types,
    );
    let order_by_param = RpcParameter::new("orderBy", RpcTypeSchema::optional(order_by_schema))
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
            // For implicit equals, get the actual scalar type from the column
            let type_name = get_scalar_type_from_column_path_link(param, subsystem);
            RpcTypeSchema::scalar(&type_name)
        }
        PredicateParameterTypeKind::Operator(operators) => {
            // Build filter type with operators like eq, neq, lt, gt, etc.
            let filter_type_name = &param_type.name;
            if !added_types.contains(filter_type_name) {
                added_types.insert(filter_type_name.clone());
                let mut filter_obj = RpcObjectType::new(filter_type_name);
                for op_param in operators {
                    // Check if this operator's type is itself a predicate type (like VectorFilterArg)
                    let op_type =
                        &subsystem.core_subsystem.predicate_types[op_param.typ.innermost().type_id];
                    let op_schema = match &op_type.kind {
                        PredicateParameterTypeKind::Vector => {
                            // Build the vector filter argument schema (for similarity search)
                            RpcTypeSchema::optional(build_vector_filter_arg_schema(
                                schema,
                                added_types,
                            ))
                        }
                        _ => {
                            // Regular scalar operator - get the type from column path link
                            let type_name =
                                get_scalar_type_from_column_path_link(op_param, subsystem);
                            RpcTypeSchema::optional(RpcTypeSchema::scalar(&type_name))
                        }
                    };
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
                // Note: 'and' and 'or' take arrays of predicates, but 'not' takes a single predicate
                for logical_param in logical_op_params {
                    let logical_schema = if logical_param.name == "not" {
                        // 'not' takes a single optional predicate
                        RpcTypeSchema::optional(RpcTypeSchema::object(filter_type_name))
                    } else {
                        // 'and' and 'or' take arrays of predicates
                        RpcTypeSchema::optional(RpcTypeSchema::array(RpcTypeSchema::object(
                            filter_type_name,
                        )))
                    };
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
            // Vector similarity search filter argument (used by 'similar' operator)
            build_vector_filter_arg_schema(schema, added_types)
        }
    }
}

fn build_pk_param_schema(
    param: &PredicateParameter,
    subsystem: &PostgresRpcSubsystemWithRouter,
) -> RpcTypeSchema {
    // PK parameters use implicit equal semantics - they're required scalar values
    // Get the actual type from the column path link
    let type_name = get_scalar_type_from_column_path_link(param, subsystem);
    RpcTypeSchema::scalar(&type_name)
}

/// Get the scalar type name from a predicate parameter.
///
/// First tries to get the type from `column_path_link` (for field parameters).
/// Falls back to the parameter type wrapper's name (for operator parameters like eq, neq, etc.).
fn get_scalar_type_from_column_path_link(
    param: &PredicateParameter,
    subsystem: &PostgresRpcSubsystemWithRouter,
) -> String {
    use exo_sql::ColumnPathLink;

    // First try to get the type from the column path link
    if let Some(ColumnPathLink::Leaf(column_id)) = &param.column_path_link {
        let column = column_id.get_column(&subsystem.core_subsystem.database);
        return column.typ.type_name().to_string();
    }

    // Fall back to the parameter type wrapper's name
    // This works for operator parameters where the type name is set directly
    // (e.g., "Int" for IntFilter's eq, neq, etc. operators)
    let param_type_wrapper = param.typ.innermost();
    param_type_wrapper.name.clone()
}

/// Build the schema for an orderBy parameter.
fn build_order_by_param_schema(
    param: &OrderByParameter,
    subsystem: &PostgresRpcSubsystemWithRouter,
    schema: &mut RpcSchema,
    added_types: &mut HashSet<String>,
) -> RpcTypeSchema {
    let param_type = &subsystem.core_subsystem.order_by_types[param.typ.innermost().type_id];

    match &param_type.kind {
        OrderByParameterTypeKind::Primitive => {
            // Primitive ordering is an enum with ASC/DESC values
            ensure_ordering_enum_added(schema, added_types);
            RpcTypeSchema::object(ORDERING_ENUM_NAME)
        }
        OrderByParameterTypeKind::Vector => {
            // Vector ordering has distance function and target vector
            ensure_vector_ordering_added(schema, added_types);
            RpcTypeSchema::object("VectorOrdering")
        }
        OrderByParameterTypeKind::Composite { parameters } => {
            // Composite ordering like ConcertOrdering with fields for each orderable field
            let ordering_type_name = &param_type.name;
            if !added_types.contains(ordering_type_name) {
                added_types.insert(ordering_type_name.clone());
                let mut ordering_obj = RpcObjectType::new(ordering_type_name);

                for field_param in parameters {
                    let field_schema = RpcTypeSchema::optional(build_order_by_param_schema(
                        field_param,
                        subsystem,
                        schema,
                        added_types,
                    ));
                    ordering_obj = ordering_obj
                        .with_field(RpcObjectField::new(&field_param.name, field_schema));
                }

                schema.add_object_type(ordering_type_name.clone(), ordering_obj);
            }
            RpcTypeSchema::object(ordering_type_name)
        }
    }
}

/// Ensure the primitive Ordering enum type is added to the schema.
fn ensure_ordering_enum_added(schema: &mut RpcSchema, added_types: &mut HashSet<String>) {
    if added_types.contains(ORDERING_ENUM_NAME) {
        return;
    }
    added_types.insert(ORDERING_ENUM_NAME.to_string());

    // Ordering is an enum with ASC and DESC values
    let ordering_obj =
        RpcObjectType::new(ORDERING_ENUM_NAME).with_description("Sort order direction");

    // Add enum values as fields (simplified representation)
    // In OpenRPC, this will be represented as an enum in the JSON Schema
    schema.add_object_type(
        ORDERING_ENUM_NAME.to_string(),
        ordering_obj.with_field(RpcObjectField::new(
            "direction",
            RpcTypeSchema::enum_type(
                PRIMITIVE_ORDERING_OPTIONS
                    .iter()
                    .map(|s| s.to_string())
                    .collect(),
            ),
        )),
    );
}

/// Ensure the VectorOrdering type is added to the schema.
fn ensure_vector_ordering_added(schema: &mut RpcSchema, added_types: &mut HashSet<String>) {
    const VECTOR_ORDERING_NAME: &str = "VectorOrdering";
    if added_types.contains(VECTOR_ORDERING_NAME) {
        return;
    }
    added_types.insert(VECTOR_ORDERING_NAME.to_string());

    let ordering_obj = RpcObjectType::new(VECTOR_ORDERING_NAME)
        .with_description("Vector similarity ordering")
        .with_field(RpcObjectField::new(
            "distanceTo",
            RpcTypeSchema::optional(RpcTypeSchema::array(RpcTypeSchema::scalar("Float"))),
        ))
        .with_field(RpcObjectField::new(
            "direction",
            RpcTypeSchema::optional(RpcTypeSchema::enum_type(
                PRIMITIVE_ORDERING_OPTIONS
                    .iter()
                    .map(|s| s.to_string())
                    .collect(),
            )),
        ));

    schema.add_object_type(VECTOR_ORDERING_NAME.to_string(), ordering_obj);
}

/// Build the VectorFilterArg schema (used by the 'similar' operator in VectorFilter).
/// VectorFilterArg has two fields:
/// - distanceTo: the target vector to compare against (array of floats)
/// - distance: the maximum distance threshold (float)
fn build_vector_filter_arg_schema(
    schema: &mut RpcSchema,
    added_types: &mut HashSet<String>,
) -> RpcTypeSchema {
    const VECTOR_FILTER_ARG_NAME: &str = "VectorFilterArg";
    if !added_types.contains(VECTOR_FILTER_ARG_NAME) {
        added_types.insert(VECTOR_FILTER_ARG_NAME.to_string());

        let filter_arg_obj = RpcObjectType::new(VECTOR_FILTER_ARG_NAME)
            .with_description("Vector similarity search argument")
            .with_field(RpcObjectField::new(
                "distanceTo",
                RpcTypeSchema::array(RpcTypeSchema::scalar("Float")),
            ))
            .with_field(RpcObjectField::new(
                "distance",
                RpcTypeSchema::optional(RpcTypeSchema::scalar("Float")),
            ));

        schema.add_object_type(VECTOR_FILTER_ARG_NAME.to_string(), filter_arg_obj);
    }
    RpcTypeSchema::object(VECTOR_FILTER_ARG_NAME)
}

// Integration tests for this module are in integration-tests/rpc-introspection
// as they require a full PostgresRpcSubsystemWithRouter setup.
