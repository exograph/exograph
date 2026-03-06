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
//! through all collection queries, pk queries, and unique constraint queries.
//!
//! Uses two traits to keep schema building generic:
//! - `BuildRpcMethod`: Converts a query (collection or pk) into an `RpcMethod`.
//! - `BuildRpcTypeSchema`: Converts a parameter type (predicate or order-by) into an `RpcTypeSchema`.

use core_model::types::{FieldType, OperationReturnType, TypeValidation};
use postgres_core_model::order::{
    OrderByParameter, OrderByParameterTypeKind, PRIMITIVE_ORDERING_OPTIONS,
};
use postgres_core_model::predicate::{PredicateParameter, PredicateParameterTypeKind};
use postgres_core_model::relation::PostgresRelation;
use postgres_core_model::types::{EntityType, PostgresFieldType, PostgresPrimitiveTypeKind};
use postgres_rpc_model::operation::{CollectionQuery, CollectionQueryParam, PkQuery, UniqueQuery};
use postgres_rpc_model::subsystem::PostgresRpcSubsystemWithRouter;
use rpc_introspection::schema::{
    OneOfVariant, RpcMethod, RpcObjectField, RpcObjectType, RpcParameter, RpcSchema, RpcTypeSchema,
};
use std::collections::{HashMap, HashSet};

/// Trait for converting a query into an `RpcMethod`.
trait BuildRpcMethod {
    fn build_rpc_method(
        &self,
        subsystem: &PostgresRpcSubsystemWithRouter,
        schema: &mut RpcSchema,
        added_types: &mut HashSet<String>,
    ) -> RpcMethod;
}

/// Trait for converting a parameter type into an `RpcTypeSchema`.
trait BuildRpcTypeSchema {
    fn build_rpc_type_schema(
        &self,
        subsystem: &PostgresRpcSubsystemWithRouter,
        schema: &mut RpcSchema,
        added_types: &mut HashSet<String>,
    ) -> RpcTypeSchema;
}

/// Build an RpcSchema from a PostgresRpcSubsystemWithRouter.
pub fn build_rpc_schema(subsystem: &PostgresRpcSubsystemWithRouter) -> RpcSchema {
    let mut schema = RpcSchema::new();
    let mut added_types: HashSet<String> = HashSet::new();

    for (_, query) in subsystem.collection_queries.iter() {
        let method = query.build_rpc_method(subsystem, &mut schema, &mut added_types);
        schema.add_method(method);
    }

    // Build merged get_<entity> methods from PK + unique queries.
    // Group unique queries by their user-facing method name.
    let mut unique_queries_by_method: HashMap<&str, Vec<&UniqueQuery>> = HashMap::new();
    for (_, query) in subsystem.unique_queries.iter() {
        unique_queries_by_method
            .entry(&query.name)
            .or_default()
            .push(query);
    }

    // Track which method names we've already added (to avoid duplicates)
    let mut added_methods: HashSet<String> = HashSet::new();

    for (_, pk_query) in subsystem.pk_queries.iter() {
        let unique_queries = unique_queries_by_method
            .remove(pk_query.name.as_str())
            .unwrap_or_default();
        let method = build_merged_get_method(
            Some(pk_query),
            &unique_queries,
            subsystem,
            &mut schema,
            &mut added_types,
        );
        schema.add_method(method);
        added_methods.insert(pk_query.name.clone());
    }

    // Handle unique queries for entities that might not have PK queries.
    // Sort by method name for deterministic output.
    let mut remaining: Vec<_> = unique_queries_by_method.into_iter().collect();
    remaining.sort_by_key(|(name, _)| *name);
    for (method_name, unique_queries) in remaining {
        if !added_methods.contains(method_name) {
            let method = build_merged_get_method(
                None,
                &unique_queries,
                subsystem,
                &mut schema,
                &mut added_types,
            );
            schema.add_method(method);
        }
    }

    schema
}

/// Build a merged RPC method that combines PK and unique constraint params.
/// Uses a single required `by` param with a `oneOf` schema listing each valid lookup group.
fn build_merged_get_method(
    pk_query: Option<&PkQuery>,
    unique_queries: &[&UniqueQuery],
    subsystem: &PostgresRpcSubsystemWithRouter,
    schema: &mut RpcSchema,
    added_types: &mut HashSet<String>,
) -> RpcMethod {
    // Use the PK query's return type/name, or fall back to the first unique query's
    let (method_name, return_type) = if let Some(pk) = pk_query {
        (&pk.name, &pk.return_type)
    } else {
        let uq = unique_queries[0];
        (&uq.name, &uq.return_type)
    };

    let result_schema = build_return_type_schema(return_type, subsystem, schema, added_types);

    let entity_name = &return_type.typ(&subsystem.core_subsystem.entity_types).name;

    // Sort unique queries by param names for deterministic output
    let mut sorted_unique_queries = unique_queries.to_vec();
    sorted_unique_queries.sort_by_key(|q| {
        q.parameters
            .predicate_params
            .iter()
            .map(|p| p.name.clone())
            .collect::<Vec<_>>()
    });

    // Build the list of lookup groups for the description.
    let mut groups: Vec<Vec<String>> = Vec::new();

    if let Some(pk) = pk_query {
        let pk_group: Vec<String> = pk
            .parameters
            .predicate_params
            .iter()
            .map(|p| p.name.clone())
            .collect();
        groups.push(pk_group);
    }
    for uq in &sorted_unique_queries {
        let uq_group: Vec<String> = uq
            .parameters
            .predicate_params
            .iter()
            .map(|p| p.name.clone())
            .collect();
        groups.push(uq_group);
    }

    // Method description lists all valid lookup groups
    let description = if groups.len() <= 1 && pk_query.is_some() {
        postgres_core_model::doc_comments::pk_query_description(entity_name)
    } else if groups.len() <= 1 {
        postgres_core_model::doc_comments::unique_query_description(entity_name)
    } else {
        let groups_str = groups
            .iter()
            .map(|g| format!("({})", g.join(", ")))
            .collect::<Vec<_>>()
            .join(", ");
        format!("Get a single `{entity_name}`. Provide one of: {groups_str}")
    };

    // Build oneOf variants for the `by` param
    let mut variants: Vec<OneOfVariant> = Vec::new();

    // PK variant
    if let Some(pk) = pk_query {
        let properties: Vec<(String, RpcTypeSchema)> = pk
            .parameters
            .predicate_params
            .iter()
            .map(|p| {
                let param_schema = build_pk_param_schema(p, subsystem);
                (p.name.clone(), param_schema)
            })
            .collect();
        let required: Vec<String> = properties.iter().map(|(n, _)| n.clone()).collect();
        variants.push(OneOfVariant {
            properties,
            required,
        });
    }

    // Unique constraint variants
    for unique_query in &sorted_unique_queries {
        let properties: Vec<(String, RpcTypeSchema)> = unique_query
            .parameters
            .predicate_params
            .iter()
            .map(|p| {
                let param_schema = build_unique_param_schema(p, subsystem, schema, added_types);
                (p.name.clone(), param_schema)
            })
            .collect();
        let required: Vec<String> = properties.iter().map(|(n, _)| n.clone()).collect();
        variants.push(OneOfVariant {
            properties,
            required,
        });
    }

    let by_schema = RpcTypeSchema::one_of(variants);
    let by_param = RpcParameter::new("by", by_schema);

    RpcMethod::new(method_name.clone(), result_schema)
        .with_description(&description)
        .with_param(by_param)
}

impl BuildRpcMethod for CollectionQuery {
    fn build_rpc_method(
        &self,
        subsystem: &PostgresRpcSubsystemWithRouter,
        schema: &mut RpcSchema,
        added_types: &mut HashSet<String>,
    ) -> RpcMethod {
        let entity_type = self.return_type.typ(&subsystem.core_subsystem.entity_types);
        let result_schema =
            build_return_type_schema(&self.return_type, subsystem, schema, added_types);

        let mut method = RpcMethod::new(self.name.clone(), result_schema);
        if let Some(doc) = &self.doc_comments {
            method = method.with_description(doc);
        }

        // Add parameters by iterating over the model's parameter list
        for param in self.parameters.params() {
            let rpc_param = match param {
                CollectionQueryParam::Predicate(p) => RpcParameter::new(
                    &p.name,
                    RpcTypeSchema::optional(p.build_rpc_type_schema(
                        subsystem,
                        schema,
                        added_types,
                    )),
                )
                // TODO: Move this to the model?
                .with_description(format!("Filter conditions for {}", entity_type.plural_name)),

                CollectionQueryParam::OrderBy(p) => RpcParameter::new(
                    &p.name,
                    RpcTypeSchema::optional(RpcTypeSchema::array(p.build_rpc_type_schema(
                        subsystem,
                        schema,
                        added_types,
                    ))),
                )
                .with_description("Ordering for the results"),

                CollectionQueryParam::Scalar(p) => RpcParameter::new(
                    &p.name,
                    RpcTypeSchema::optional(RpcTypeSchema::scalar(&p.type_name)),
                )
                .with_description(&p.description),
            };
            method = method.with_param(rpc_param);
        }

        method
    }
}

impl BuildRpcTypeSchema for PredicateParameter {
    fn build_rpc_type_schema(
        &self,
        subsystem: &PostgresRpcSubsystemWithRouter,
        schema: &mut RpcSchema,
        added_types: &mut HashSet<String>,
    ) -> RpcTypeSchema {
        let param_type = &subsystem.core_subsystem.predicate_types[self.typ.innermost().type_id];

        match &param_type.kind {
            PredicateParameterTypeKind::ImplicitEqual => {
                let type_name = get_scalar_type_from_column_path_link(self, subsystem);
                RpcTypeSchema::scalar(&type_name)
            }
            PredicateParameterTypeKind::Operator(operators) => {
                let filter_type_name = &param_type.name;
                if !added_types.contains(filter_type_name) {
                    added_types.insert(filter_type_name.clone());
                    let mut filter_obj = RpcObjectType::new(filter_type_name);
                    for op_param in operators {
                        let op_type = &subsystem.core_subsystem.predicate_types
                            [op_param.typ.innermost().type_id];
                        let op_schema = match &op_type.kind {
                            PredicateParameterTypeKind::Vector => RpcTypeSchema::optional(
                                build_vector_filter_arg_schema(schema, added_types),
                            ),
                            _ => {
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
                let filter_type_name = &param_type.name;
                if !added_types.contains(filter_type_name) {
                    added_types.insert(filter_type_name.clone());
                    let mut filter_obj = RpcObjectType::new(filter_type_name);

                    for field_param in field_params {
                        let field_schema = RpcTypeSchema::optional(
                            field_param.build_rpc_type_schema(subsystem, schema, added_types),
                        );
                        filter_obj = filter_obj
                            .with_field(RpcObjectField::new(&field_param.name, field_schema));
                    }

                    // 'and' and 'or' take arrays of predicates, but 'not' takes a single predicate
                    for logical_param in logical_op_params {
                        let logical_schema = if logical_param.name == "not" {
                            RpcTypeSchema::optional(RpcTypeSchema::object(filter_type_name))
                        } else {
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
                let ref_filter_name = &param_type.name;
                if !added_types.contains(ref_filter_name) {
                    added_types.insert(ref_filter_name.clone());
                    let mut filter_obj = RpcObjectType::new(ref_filter_name);
                    for ref_param in ref_params {
                        let ref_schema = RpcTypeSchema::optional(ref_param.build_rpc_type_schema(
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
                build_vector_filter_arg_schema(schema, added_types)
            }
        }
    }
}

impl BuildRpcTypeSchema for OrderByParameter {
    fn build_rpc_type_schema(
        &self,
        subsystem: &PostgresRpcSubsystemWithRouter,
        schema: &mut RpcSchema,
        added_types: &mut HashSet<String>,
    ) -> RpcTypeSchema {
        let param_type = &subsystem.core_subsystem.order_by_types[self.typ.innermost().type_id];

        match &param_type.kind {
            OrderByParameterTypeKind::Primitive => RpcTypeSchema::enum_type(
                PRIMITIVE_ORDERING_OPTIONS
                    .iter()
                    .map(|s| s.to_string())
                    .collect(),
            ),
            OrderByParameterTypeKind::Vector => {
                ensure_vector_ordering_added(schema, added_types);
                RpcTypeSchema::object("VectorOrdering")
            }
            OrderByParameterTypeKind::Composite { parameters } => {
                let ordering_type_name = &param_type.name;
                if !added_types.contains(ordering_type_name) {
                    added_types.insert(ordering_type_name.clone());
                    let mut ordering_obj = RpcObjectType::new(ordering_type_name);

                    for field_param in parameters {
                        let field_schema = RpcTypeSchema::optional(
                            field_param.build_rpc_type_schema(subsystem, schema, added_types),
                        );
                        ordering_obj = ordering_obj
                            .with_field(RpcObjectField::new(&field_param.name, field_schema));
                    }

                    schema.add_object_type(ordering_type_name.clone(), ordering_obj);
                }
                RpcTypeSchema::object(ordering_type_name)
            }
        }
    }
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
        match &field.relation {
            PostgresRelation::Scalar { .. } => {
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
            PostgresRelation::ManyToOne { relation, .. } => {
                let foreign_entity =
                    &subsystem.core_subsystem.entity_types[relation.foreign_entity_id];

                // Build a PK-only reference type (e.g., "UserRef")
                let ref_type_name = format!("{}Ref", foreign_entity.name);
                if !added_types.contains(&ref_type_name) {
                    added_types.insert(ref_type_name.clone());
                    let mut ref_obj = RpcObjectType::new(&ref_type_name);
                    for pk_field in foreign_entity.pk_fields() {
                        if let PostgresRelation::Scalar { .. } = pk_field.relation {
                            let pk_schema = build_field_type_schema(
                                &pk_field.typ,
                                pk_field.type_validation.as_ref(),
                                subsystem,
                                schema,
                                added_types,
                            );
                            ref_obj =
                                ref_obj.with_field(RpcObjectField::new(&pk_field.name, pk_schema));
                        }
                    }
                    schema.add_object_type(ref_type_name.clone(), ref_obj);
                }

                // Handle optional relations
                let ref_schema = match &field.typ {
                    FieldType::Optional(_) => {
                        RpcTypeSchema::optional(RpcTypeSchema::object(&ref_type_name))
                    }
                    _ => RpcTypeSchema::object(&ref_type_name),
                };

                let mut obj_field = RpcObjectField::new(&field.name, ref_schema);
                if let Some(doc) = &field.doc_comments {
                    obj_field = obj_field.with_description(doc);
                }
                obj_type = obj_type.with_field(obj_field);
            }
            _ => {}
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

fn build_pk_param_schema(
    param: &PredicateParameter,
    subsystem: &PostgresRpcSubsystemWithRouter,
) -> RpcTypeSchema {
    // PK parameters use implicit equal semantics - they're required scalar values
    let type_name = get_scalar_type_from_column_path_link(param, subsystem);
    RpcTypeSchema::scalar(&type_name)
}

/// Build schema for a unique constraint parameter.
/// Scalar fields use implicit equal; ManyToOne relations use their unique filter type.
fn build_unique_param_schema(
    param: &PredicateParameter,
    subsystem: &PostgresRpcSubsystemWithRouter,
    schema: &mut RpcSchema,
    added_types: &mut HashSet<String>,
) -> RpcTypeSchema {
    let param_type = &subsystem.core_subsystem.predicate_types[param.typ.innermost().type_id];

    match &param_type.kind {
        PredicateParameterTypeKind::ImplicitEqual => {
            let type_name = get_scalar_type_from_column_path_link(param, subsystem);
            RpcTypeSchema::scalar(&type_name)
        }
        PredicateParameterTypeKind::Reference(ref_params) => {
            // ManyToOne relation - build the reference filter object type
            let ref_filter_name = &param_type.name;
            if !added_types.contains(ref_filter_name) {
                added_types.insert(ref_filter_name.clone());
                let mut filter_obj = RpcObjectType::new(ref_filter_name);
                for ref_param in ref_params {
                    let ref_schema = RpcTypeSchema::optional(ref_param.build_rpc_type_schema(
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
        _ => {
            // Fallback: use the predicate parameter's existing schema building
            param.build_rpc_type_schema(subsystem, schema, added_types)
        }
    }
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
    let param_type_wrapper = param.typ.innermost();
    param_type_wrapper.name.clone()
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
    // TODO: Unify with other ways we handle parameter types (and then have GraphQL also use the same way)
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
