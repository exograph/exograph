// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use core_model::types::{FieldType, OperationReturnType, TypeValidation};
use postgres_core_model::projection::{
    PROJECTION_BASIC, PROJECTION_PK, ProjectionElement, ResolvedProjection,
};
use postgres_core_model::relation::PostgresRelation;
use postgres_core_model::types::{EntityType, PostgresFieldType, PostgresPrimitiveTypeKind};
use postgres_rpc_model::subsystem::PostgresRpcSubsystemWithRouter;
use rpc_introspection::schema::{RpcObjectField, RpcObjectType, RpcSchema, RpcTypeSchema};
use std::collections::HashSet;

/// Shared recursive return type schema builder.
pub(crate) fn build_return_type_schema_with(
    return_type: &OperationReturnType<EntityType>,
    projection_name: &str,
    subsystem: &PostgresRpcSubsystemWithRouter,
    schema: &mut RpcSchema,
    added_types: &mut HashSet<String>,
) -> RpcTypeSchema {
    match return_type {
        OperationReturnType::Plain(base) => {
            let entity_type = &subsystem.core_subsystem.entity_types[base.associated_type_id];

            let projection = entity_type.projection_by_name(projection_name);

            let projection = projection.unwrap_or_else(|| {
                panic!(
                    "Projection `{}` not found for type `{}`",
                    projection_name, entity_type.name
                )
            });

            let type_name = projection_type_name(&entity_type.name, &projection.name);
            ensure_projection_type_added(
                &type_name,
                entity_type,
                projection,
                subsystem,
                schema,
                added_types,
            );
            RpcTypeSchema::object(&type_name)
        }
        OperationReturnType::Optional(inner) => RpcTypeSchema::optional(
            build_return_type_schema_with(inner, projection_name, subsystem, schema, added_types),
        ),
        OperationReturnType::List(inner) => RpcTypeSchema::array(build_return_type_schema_with(
            inner,
            projection_name,
            subsystem,
            schema,
            added_types,
        )),
    }
}

fn pk_type_name(entity_name: &str) -> String {
    format!("{entity_name}PK")
}

pub(crate) fn ensure_entity_type_added(
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
        let add_field = |obj_type: RpcObjectType, field_schema: RpcTypeSchema| {
            let mut obj_field = RpcObjectField::new(&field.name, field_schema);
            if let Some(doc) = &field.doc_comments {
                obj_field = obj_field.with_description(doc);
            }
            obj_type.with_field(obj_field)
        };

        match &field.relation {
            PostgresRelation::Scalar { .. } => {
                let field_schema = build_field_type_schema(
                    &field.typ,
                    field.type_validation.as_ref(),
                    subsystem,
                    schema,
                    added_types,
                );

                obj_type = add_field(obj_type, field_schema);
            }
            PostgresRelation::ManyToOne { relation, .. } => {
                let foreign_entity =
                    &subsystem.core_subsystem.entity_types[relation.foreign_entity_id];
                let ref_type_name =
                    ensure_ref_type_added(foreign_entity, subsystem, schema, added_types);

                // Handle optional relations
                let ref_schema = match &field.typ {
                    FieldType::Optional(_) => {
                        RpcTypeSchema::optional(RpcTypeSchema::object(&ref_type_name))
                    }
                    _ => RpcTypeSchema::object(&ref_type_name),
                };

                obj_type = add_field(obj_type, ref_schema);
            }
            _ => {}
        }
    }

    schema.add_object_type(entity_type.name.clone(), obj_type);
}

/// Ensure a PK-only reference type (e.g., "VenueRef") is added to the schema.
pub(crate) fn ensure_ref_type_added(
    foreign_entity: &EntityType,
    subsystem: &PostgresRpcSubsystemWithRouter,
    schema: &mut RpcSchema,
    added_types: &mut HashSet<String>,
) -> String {
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
                ref_obj = ref_obj.with_field(RpcObjectField::new(&pk_field.name, pk_schema));
            }
        }
        schema.add_object_type(ref_type_name.clone(), ref_obj);
    }
    ref_type_name
}

/// Generate a type name for a projection: "Concert" for basic, "ConcertPK" for pk,
/// "ConcertWithVenue" for custom projections.
fn projection_type_name(entity_name: &str, projection_name: &str) -> String {
    match projection_name {
        PROJECTION_BASIC => entity_name.to_string(),
        PROJECTION_PK => pk_type_name(entity_name),
        name => {
            let mut chars = name.chars();
            let capitalized: String = match chars.next() {
                Some(c) => c.to_uppercase().chain(chars).collect(),
                None => String::new(),
            };
            format!("{entity_name}{capitalized}")
        }
    }
}

/// Ensure a projection-based type is added to the schema.
fn ensure_projection_type_added(
    type_name: &str,
    entity_type: &EntityType,
    projection: &ResolvedProjection,
    subsystem: &PostgresRpcSubsystemWithRouter,
    schema: &mut RpcSchema,
    added_types: &mut HashSet<String>,
) {
    if added_types.contains(type_name) {
        return;
    }
    added_types.insert(type_name.to_string());

    let mut obj_type = RpcObjectType::new(type_name);

    if let Some(doc) = &entity_type.doc_comments {
        obj_type = obj_type.with_description(doc);
    }

    for element in &projection.elements {
        match element {
            ProjectionElement::ScalarField(field_name) => {
                if let Some(field) = entity_type.field_by_name(field_name) {
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
            ProjectionElement::RelationProjection {
                relation_field_name,
                projection_name,
            } => {
                if let Some(field) = entity_type.field_by_name(relation_field_name) {
                    match &field.relation {
                        PostgresRelation::ManyToOne { relation, .. } => {
                            let foreign_entity =
                                &subsystem.core_subsystem.entity_types[relation.foreign_entity_id];
                            let rel_type_name =
                                projection_type_name(&foreign_entity.name, projection_name);

                            if let Some(foreign_projection) =
                                foreign_entity.projection_by_name(projection_name)
                            {
                                ensure_projection_type_added(
                                    &rel_type_name,
                                    foreign_entity,
                                    foreign_projection,
                                    subsystem,
                                    schema,
                                    added_types,
                                );
                            }

                            let ref_schema = match &field.typ {
                                FieldType::Optional(_) => {
                                    RpcTypeSchema::optional(RpcTypeSchema::object(&rel_type_name))
                                }
                                _ => RpcTypeSchema::object(&rel_type_name),
                            };

                            let mut obj_field =
                                RpcObjectField::new(relation_field_name, ref_schema);
                            if let Some(doc) = &field.doc_comments {
                                obj_field = obj_field.with_description(doc);
                            }
                            obj_type = obj_type.with_field(obj_field);
                        }
                        PostgresRelation::OneToMany(relation) => {
                            let foreign_entity =
                                &subsystem.core_subsystem.entity_types[relation.foreign_entity_id];
                            let rel_type_name =
                                projection_type_name(&foreign_entity.name, projection_name);

                            if let Some(foreign_projection) =
                                foreign_entity.projection_by_name(projection_name)
                            {
                                ensure_projection_type_added(
                                    &rel_type_name,
                                    foreign_entity,
                                    foreign_projection,
                                    subsystem,
                                    schema,
                                    added_types,
                                );
                            }

                            let item_schema = RpcTypeSchema::object(&rel_type_name);
                            let array_schema = RpcTypeSchema::array(item_schema);

                            let mut obj_field =
                                RpcObjectField::new(relation_field_name, array_schema);
                            if let Some(doc) = &field.doc_comments {
                                obj_field = obj_field.with_description(doc);
                            }
                            obj_type = obj_type.with_field(obj_field);
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    schema.add_object_type(type_name.to_string(), obj_type);
}

pub(crate) fn build_field_type_schema(
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

/// Get the scalar type name from a predicate parameter.
///
/// First tries to get the type from `column_path_link` (for field parameters).
/// Falls back to the parameter type wrapper's name (for operator parameters like eq, neq, etc.).
pub(super) fn get_scalar_type_from_column_path_link(
    param: &postgres_core_model::predicate::PredicateParameter,
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
