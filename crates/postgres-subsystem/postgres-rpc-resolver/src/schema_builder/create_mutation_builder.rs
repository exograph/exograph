// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use core_model::types::FieldType;
use postgres_core_model::relation::{OneToManyRelation, PostgresRelation};
use postgres_core_model::types::EntityType;
use postgres_rpc_model::subsystem::PostgresRpcSubsystemWithRouter;
use rpc_introspection::schema::{
    RpcMethod, RpcObjectField, RpcObjectType, RpcParameter, RpcSchema, RpcTypeSchema,
};
use std::collections::HashSet;

use super::ReturnTypeKind;
use super::type_builder::{
    build_field_type_schema, build_return_type_schema_with, ensure_ref_type_added,
};

fn create_input_type_name(entity_name: &str) -> String {
    format!("{entity_name}CreateInput")
}

/// Build an RPC method for create operations (single or collection).
pub(super) fn build_create_method<P>(
    op: &postgres_rpc_model::operation::PostgresOperation<P>,
    data_param_name: &str,
    is_collection: bool,
    subsystem: &PostgresRpcSubsystemWithRouter,
    schema: &mut RpcSchema,
    added_types: &mut HashSet<String>,
) -> RpcMethod {
    let entity_type = op.return_type.typ(&subsystem.core_subsystem.entity_types);
    let result_schema = build_return_type_schema_with(
        &op.return_type,
        ReturnTypeKind::PkOnly,
        subsystem,
        schema,
        added_types,
    );

    let mut method = RpcMethod::new(op.name.clone(), result_schema);
    if let Some(doc) = &op.doc_comments {
        method = method.with_description(doc);
    }

    ensure_create_input_type_added(entity_type, subsystem, schema, added_types);
    let input_type_name = create_input_type_name(&entity_type.name);
    let data_schema = if is_collection {
        RpcTypeSchema::array(RpcTypeSchema::object(&input_type_name))
    } else {
        RpcTypeSchema::object(&input_type_name)
    };
    let data_param = RpcParameter::new(data_param_name, data_schema)
        .with_description(format!("Data for creating {}", entity_type.name));
    method = method.with_param(data_param);

    method
}

/// Ensure the create input type for an entity is added to the schema.
/// PK fields with autoIncrement are excluded. Other PK fields are included (optional if they have a default).
/// Non-PK fields are required unless they have a default value (then optional).
fn ensure_create_input_type_added(
    entity_type: &EntityType,
    subsystem: &PostgresRpcSubsystemWithRouter,
    schema: &mut RpcSchema,
    added_types: &mut HashSet<String>,
) {
    let type_name = create_input_type_name(&entity_type.name);

    if added_types.contains(&type_name) {
        return;
    }
    added_types.insert(type_name.clone());

    let mut create_obj = RpcObjectType::new(&type_name);

    for field in &entity_type.fields {
        match &field.relation {
            PostgresRelation::Scalar { column_id, .. } => {
                let column = column_id.get_column(&subsystem.core_subsystem.database);

                // Skip autoIncrement PK fields
                if field.relation.is_pk()
                    && column
                        .default_value
                        .as_ref()
                        .is_some_and(|d| d.is_autoincrement())
                {
                    continue;
                }

                let field_schema = build_field_type_schema(
                    &field.typ,
                    field.type_validation.as_ref(),
                    subsystem,
                    schema,
                    added_types,
                );

                // Make optional if the field has a default value or if the type is already Optional
                let final_schema = if column.default_value.is_some()
                    || matches!(&field.typ, FieldType::Optional(_))
                {
                    RpcTypeSchema::optional(field_schema)
                } else {
                    field_schema
                };

                create_obj = create_obj.with_field(RpcObjectField::new(&field.name, final_schema));
            }
            PostgresRelation::ManyToOne { relation, .. } => {
                let foreign_entity =
                    &subsystem.core_subsystem.entity_types[relation.foreign_entity_id];
                let ref_type_name =
                    ensure_ref_type_added(foreign_entity, subsystem, schema, added_types);

                // ManyToOne references are required unless the field type is Optional
                let ref_schema = match &field.typ {
                    FieldType::Optional(_) => {
                        RpcTypeSchema::optional(RpcTypeSchema::object(&ref_type_name))
                    }
                    _ => RpcTypeSchema::object(&ref_type_name),
                };
                create_obj = create_obj.with_field(RpcObjectField::new(&field.name, ref_schema));
            }
            PostgresRelation::OneToMany(one_to_many_relation) => {
                let nested_type_name = ensure_nested_create_input_type_added(
                    entity_type,
                    one_to_many_relation,
                    subsystem,
                    schema,
                    added_types,
                );

                // OneToMany nested creates are optional arrays
                let nested_schema = RpcTypeSchema::optional(RpcTypeSchema::array(
                    RpcTypeSchema::object(&nested_type_name),
                ));
                create_obj = create_obj.with_field(RpcObjectField::new(&field.name, nested_schema));
            }
            _ => {}
        }
    }

    schema.add_object_type(type_name, create_obj);
}

/// Ensure a nested create input type is added for a OneToMany relation.
/// E.g., `ConcertCreateInputFromVenue` — same as the regular create input but
/// skips the ManyToOne field that references back to the parent entity.
pub(super) fn ensure_nested_create_input_type_added(
    parent_entity: &EntityType,
    one_to_many_relation: &OneToManyRelation,
    subsystem: &PostgresRpcSubsystemWithRouter,
    schema: &mut RpcSchema,
    added_types: &mut HashSet<String>,
) -> String {
    let foreign_entity =
        &subsystem.core_subsystem.entity_types[one_to_many_relation.foreign_entity_id];
    let type_name = format!(
        "{}CreateInputFrom{}",
        foreign_entity.name, parent_entity.name
    );

    if added_types.contains(&type_name) {
        return type_name;
    }
    added_types.insert(type_name.clone());

    let mut create_obj = RpcObjectType::new(&type_name);

    for field in &foreign_entity.fields {
        // Skip ManyToOne fields that reference back to the parent entity
        if let PostgresRelation::ManyToOne { relation, .. } = &field.relation {
            let target_entity = &subsystem.core_subsystem.entity_types[relation.foreign_entity_id];
            if target_entity.table_id == parent_entity.table_id {
                continue;
            }
        }

        match &field.relation {
            PostgresRelation::Scalar { column_id, .. } => {
                let column = column_id.get_column(&subsystem.core_subsystem.database);

                // Skip autoIncrement PK fields
                if field.relation.is_pk()
                    && column
                        .default_value
                        .as_ref()
                        .is_some_and(|d| d.is_autoincrement())
                {
                    continue;
                }

                let field_schema = build_field_type_schema(
                    &field.typ,
                    field.type_validation.as_ref(),
                    subsystem,
                    schema,
                    added_types,
                );

                let final_schema = if column.default_value.is_some()
                    || matches!(&field.typ, FieldType::Optional(_))
                {
                    RpcTypeSchema::optional(field_schema)
                } else {
                    field_schema
                };

                create_obj = create_obj.with_field(RpcObjectField::new(&field.name, final_schema));
            }
            PostgresRelation::ManyToOne { relation, .. } => {
                let ref_entity = &subsystem.core_subsystem.entity_types[relation.foreign_entity_id];
                let ref_type_name =
                    ensure_ref_type_added(ref_entity, subsystem, schema, added_types);

                let ref_schema = match &field.typ {
                    FieldType::Optional(_) => {
                        RpcTypeSchema::optional(RpcTypeSchema::object(&ref_type_name))
                    }
                    _ => RpcTypeSchema::object(&ref_type_name),
                };
                create_obj = create_obj.with_field(RpcObjectField::new(&field.name, ref_schema));
            }
            PostgresRelation::OneToMany(nested_one_to_many) => {
                let nested_type_name = ensure_nested_create_input_type_added(
                    foreign_entity,
                    nested_one_to_many,
                    subsystem,
                    schema,
                    added_types,
                );

                let nested_schema = RpcTypeSchema::optional(RpcTypeSchema::array(
                    RpcTypeSchema::object(&nested_type_name),
                ));
                create_obj = create_obj.with_field(RpcObjectField::new(&field.name, nested_schema));
            }
            _ => {}
        }
    }

    schema.add_object_type(type_name.clone(), create_obj);
    type_name
}
