// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use postgres_core_model::order::{
    OrderByParameter, OrderByParameterTypeKind, PRIMITIVE_ORDERING_OPTIONS,
};
use postgres_core_model::predicate::{PredicateParameter, PredicateParameterTypeKind};
use postgres_rpc_model::operation::{CollectionQuery, CollectionQueryParam};
use postgres_rpc_model::subsystem::PostgresRpcSubsystem;
use rpc_introspection::schema::{
    RpcMethod, RpcObjectField, RpcObjectType, RpcParameter, RpcSchema, RpcTypeSchema,
};
use std::collections::HashSet;

use super::type_builder::{
    build_return_type_schema_for_entity, get_scalar_type_from_column_path_link,
};
use super::{BuildRpcMethod, BuildRpcTypeSchema, build_projection_param};

impl BuildRpcMethod for CollectionQuery {
    fn build_rpc_method(
        &self,
        subsystem: &PostgresRpcSubsystem,
        schema: &mut RpcSchema,
        added_types: &mut HashSet<String>,
    ) -> RpcMethod {
        let entity_type = self.return_type.typ(&subsystem.core_subsystem.entity_types);

        let result_schema =
            build_return_type_schema_for_entity(&self.return_type, subsystem, schema, added_types);

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

        method = method.with_param(build_projection_param(entity_type));

        method
    }
}

impl BuildRpcTypeSchema for PredicateParameter {
    fn build_rpc_type_schema(
        &self,
        subsystem: &PostgresRpcSubsystem,
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
        subsystem: &PostgresRpcSubsystem,
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
