// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use postgres_core_model::relation::{OneToManyRelation, PostgresRelation};
use postgres_core_model::types::EntityType;
use postgres_rpc_model::operation::{CollectionUpdate, HasPredicateParams};
use postgres_rpc_model::subsystem::PostgresRpcSubsystemWithRouter;
use rpc_introspection::schema::{
    RpcMethod, RpcObjectField, RpcObjectType, RpcParameter, RpcSchema, RpcTypeSchema,
};
use std::collections::HashSet;

use super::create_mutation_builder::ensure_nested_create_input_type_added;
use super::type_builder::{
    build_field_type_schema, build_return_type_schema_with, ensure_ref_type_added,
};
use super::{
    BuildRpcMethod, BuildRpcTypeSchema, HasMethodNameAndReturnType, ReturnTypeKind,
    build_predicate_params_method,
};

fn update_input_type_name(entity_name: &str) -> String {
    format!("{entity_name}UpdateInput")
}

impl BuildRpcMethod for CollectionUpdate {
    fn build_rpc_method(
        &self,
        subsystem: &PostgresRpcSubsystemWithRouter,
        schema: &mut RpcSchema,
        added_types: &mut HashSet<String>,
    ) -> RpcMethod {
        let entity_type = self.return_type.typ(&subsystem.core_subsystem.entity_types);
        let result_schema = build_return_type_schema_with(
            &self.return_type,
            ReturnTypeKind::PkOnly,
            subsystem,
            schema,
            added_types,
        );

        let mut method = RpcMethod::new(self.name.clone(), result_schema);
        if let Some(doc) = &self.doc_comments {
            method = method.with_description(doc);
        }

        // Add `where` parameter
        let where_param = RpcParameter::new(
            &self.parameters.predicate_param.name,
            RpcTypeSchema::optional(self.parameters.predicate_param.build_rpc_type_schema(
                subsystem,
                schema,
                added_types,
            )),
        )
        .with_description(format!("Filter conditions for {}", entity_type.plural_name));
        method = method.with_param(where_param);

        // Add `data` parameter
        method = append_update_data_param(
            method,
            &self.parameters.data_param.name,
            entity_type,
            &format!("Data to update for matching {}", entity_type.plural_name),
            subsystem,
            schema,
            added_types,
        );

        method
    }
}

/// Build an RPC method from an update operation with predicate params (PK or unique).
pub(super) fn build_update_predicate_params_method<T>(
    op: &T,
    data_param_name: &str,
    subsystem: &PostgresRpcSubsystemWithRouter,
    schema: &mut RpcSchema,
    added_types: &mut HashSet<String>,
) -> RpcMethod
where
    T: HasPredicateParams + HasMethodNameAndReturnType,
{
    let mut method =
        build_predicate_params_method(op, ReturnTypeKind::PkOnly, subsystem, schema, added_types);

    // Append `data` parameter
    let entity_type = op.return_type().typ(&subsystem.core_subsystem.entity_types);
    method = append_update_data_param(
        method,
        data_param_name,
        entity_type,
        &format!("Data to update for {}", entity_type.name),
        subsystem,
        schema,
        added_types,
    );

    method
}

/// Append the `data` parameter to an RPC method for update operations.
fn append_update_data_param(
    method: RpcMethod,
    data_param_name: &str,
    entity_type: &EntityType,
    description: &str,
    subsystem: &PostgresRpcSubsystemWithRouter,
    schema: &mut RpcSchema,
    added_types: &mut HashSet<String>,
) -> RpcMethod {
    ensure_update_input_type_added(entity_type, subsystem, schema, added_types);
    let data_param = RpcParameter::new(
        data_param_name,
        RpcTypeSchema::object(update_input_type_name(&entity_type.name)),
    )
    .with_description(description);
    method.with_param(data_param)
}

/// Ensure the update input type for an entity is added to the schema.
/// All non-PK scalar fields are optional.
fn ensure_update_input_type_added(
    entity_type: &EntityType,
    subsystem: &PostgresRpcSubsystemWithRouter,
    schema: &mut RpcSchema,
    added_types: &mut HashSet<String>,
) {
    let type_name = update_input_type_name(&entity_type.name);

    if added_types.contains(&type_name) {
        return;
    }
    added_types.insert(type_name.clone());

    let mut update_obj = RpcObjectType::new(&type_name);

    for field in &entity_type.fields {
        // Skip PK fields
        if field.relation.is_pk() {
            continue;
        }

        match &field.relation {
            PostgresRelation::Scalar { .. } => {
                let field_schema = build_field_type_schema(
                    &field.typ,
                    field.type_validation.as_ref(),
                    subsystem,
                    schema,
                    added_types,
                );
                // All update fields are optional
                let optional_schema = RpcTypeSchema::optional(field_schema);
                update_obj =
                    update_obj.with_field(RpcObjectField::new(&field.name, optional_schema));
            }
            PostgresRelation::ManyToOne { relation, .. } => {
                let foreign_entity =
                    &subsystem.core_subsystem.entity_types[relation.foreign_entity_id];
                let ref_type_name =
                    ensure_ref_type_added(foreign_entity, subsystem, schema, added_types);

                // ManyToOne references are always optional in update input
                let ref_schema = RpcTypeSchema::optional(RpcTypeSchema::object(&ref_type_name));
                update_obj = update_obj.with_field(RpcObjectField::new(&field.name, ref_schema));
            }
            PostgresRelation::OneToMany(one_to_many_relation) => {
                let wrapper_type_name = ensure_update_ops_type_added(
                    entity_type,
                    one_to_many_relation,
                    subsystem,
                    schema,
                    added_types,
                );

                let ops_schema = RpcTypeSchema::optional(RpcTypeSchema::object(&wrapper_type_name));
                update_obj = update_obj.with_field(RpcObjectField::new(&field.name, ops_schema));
            }
            _ => {}
        }
    }

    schema.add_object_type(type_name, update_obj);
}

/// Ensure the update ops wrapper type is added for a OneToMany relation.
/// E.g., `ConcertUpdateOpsFromVenue` with create/update/delete sub-fields.
fn ensure_update_ops_type_added(
    parent_entity: &EntityType,
    one_to_many_relation: &OneToManyRelation,
    subsystem: &PostgresRpcSubsystemWithRouter,
    schema: &mut RpcSchema,
    added_types: &mut HashSet<String>,
) -> String {
    let foreign_entity =
        &subsystem.core_subsystem.entity_types[one_to_many_relation.foreign_entity_id];
    let wrapper_type_name = format!("{}UpdateOpsFrom{}", foreign_entity.name, parent_entity.name);

    if added_types.contains(&wrapper_type_name) {
        return wrapper_type_name;
    }
    added_types.insert(wrapper_type_name.clone());

    // Reuse the nested create input type from create_mutation_builder
    let create_type_name = ensure_nested_create_input_type_added(
        parent_entity,
        one_to_many_relation,
        subsystem,
        schema,
        added_types,
    );

    // Create the nested update input type (includes PKs as required)
    let update_type_name = ensure_nested_update_input_type_added(
        parent_entity,
        one_to_many_relation,
        subsystem,
        schema,
        added_types,
    );

    // Reuse existing ref type for delete
    let ref_type_name = ensure_ref_type_added(foreign_entity, subsystem, schema, added_types);

    let wrapper_obj = RpcObjectType::new(&wrapper_type_name)
        .with_field(RpcObjectField::new(
            "create",
            RpcTypeSchema::optional(RpcTypeSchema::array(RpcTypeSchema::object(
                &create_type_name,
            ))),
        ))
        .with_field(RpcObjectField::new(
            "update",
            RpcTypeSchema::optional(RpcTypeSchema::array(RpcTypeSchema::object(
                &update_type_name,
            ))),
        ))
        .with_field(RpcObjectField::new(
            "delete",
            RpcTypeSchema::optional(RpcTypeSchema::array(RpcTypeSchema::object(&ref_type_name))),
        ));

    schema.add_object_type(wrapper_type_name.clone(), wrapper_obj);
    wrapper_type_name
}

/// Ensure a nested update input type is added for a OneToMany relation.
/// E.g., `ConcertNestedUpdateInputFromVenue` — includes PK fields as required
/// (for identifying which row to update) and non-PK fields as optional.
/// Skips ManyToOne fields that reference back to the parent entity.
fn ensure_nested_update_input_type_added(
    parent_entity: &EntityType,
    one_to_many_relation: &OneToManyRelation,
    subsystem: &PostgresRpcSubsystemWithRouter,
    schema: &mut RpcSchema,
    added_types: &mut HashSet<String>,
) -> String {
    let foreign_entity =
        &subsystem.core_subsystem.entity_types[one_to_many_relation.foreign_entity_id];
    let type_name = format!(
        "{}NestedUpdateInputFrom{}",
        foreign_entity.name, parent_entity.name
    );

    if added_types.contains(&type_name) {
        return type_name;
    }
    added_types.insert(type_name.clone());

    let mut update_obj = RpcObjectType::new(&type_name);

    for field in &foreign_entity.fields {
        // Skip ManyToOne fields that reference back to the parent entity
        if let PostgresRelation::ManyToOne { relation, .. } = &field.relation {
            let target_entity = &subsystem.core_subsystem.entity_types[relation.foreign_entity_id];
            if target_entity.table_id == parent_entity.table_id {
                continue;
            }
        }

        match &field.relation {
            PostgresRelation::Scalar { .. } => {
                let field_schema = build_field_type_schema(
                    &field.typ,
                    field.type_validation.as_ref(),
                    subsystem,
                    schema,
                    added_types,
                );

                // PK fields are required (for identification), non-PK fields are optional
                let final_schema = if field.relation.is_pk() {
                    field_schema
                } else {
                    RpcTypeSchema::optional(field_schema)
                };

                update_obj = update_obj.with_field(RpcObjectField::new(&field.name, final_schema));
            }
            PostgresRelation::ManyToOne { relation, .. } => {
                let ref_entity = &subsystem.core_subsystem.entity_types[relation.foreign_entity_id];
                let ref_type_name =
                    ensure_ref_type_added(ref_entity, subsystem, schema, added_types);

                let ref_schema = RpcTypeSchema::optional(RpcTypeSchema::object(&ref_type_name));
                update_obj = update_obj.with_field(RpcObjectField::new(&field.name, ref_schema));
            }
            _ => {}
        }
    }

    schema.add_object_type(type_name.clone(), update_obj);
    type_name
}
