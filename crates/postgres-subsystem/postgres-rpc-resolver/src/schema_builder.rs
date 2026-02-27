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
use postgres_rpc_model::operation::{CollectionQuery, CollectionQueryParam, PkQuery};
use postgres_rpc_model::subsystem::PostgresRpcSubsystemWithRouter;
use rpc_introspection::schema::{
    RpcMethod, RpcObjectField, RpcObjectType, RpcParameter, RpcSchema, RpcTypeSchema,
};
use std::collections::HashSet;

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

    for (_, query) in subsystem.pk_queries.iter() {
        let method = query.build_rpc_method(subsystem, &mut schema, &mut added_types);
        schema.add_method(method);
    }

    schema
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

impl BuildRpcMethod for PkQuery {
    fn build_rpc_method(
        &self,
        subsystem: &PostgresRpcSubsystemWithRouter,
        schema: &mut RpcSchema,
        added_types: &mut HashSet<String>,
    ) -> RpcMethod {
        let result_schema =
            build_return_type_schema(&self.return_type, subsystem, schema, added_types);

        let mut method = RpcMethod::new(self.name.clone(), result_schema);
        if let Some(doc) = &self.doc_comments {
            method = method.with_description(doc);
        }

        // Add pk parameters (all required)
        for predicate_param in &self.parameters.predicate_params {
            let param_schema = build_pk_param_schema(predicate_param, subsystem);
            let param = RpcParameter::new(&predicate_param.name, param_schema)
                .with_description(format!("Primary key field: {}", predicate_param.name));
            method = method.with_param(param);
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

fn build_pk_param_schema(
    param: &PredicateParameter,
    subsystem: &PostgresRpcSubsystemWithRouter,
) -> RpcTypeSchema {
    // PK parameters use implicit equal semantics - they're required scalar values
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
