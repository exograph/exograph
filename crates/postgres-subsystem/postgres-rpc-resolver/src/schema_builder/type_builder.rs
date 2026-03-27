// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use core_model::types::{FieldType, OperationReturnType, TypeValidation};
use indexmap::IndexMap;
use postgres_core_model::projection::{
    PROJECTION_BASIC, PROJECTION_PK, ProjectionElement, ResolvedProjection,
};
use postgres_core_model::relation::PostgresRelation;
use postgres_core_model::types::{EntityType, PostgresFieldType, PostgresPrimitiveTypeKind};
use postgres_rpc_model::subsystem::PostgresRpcSubsystemWithRouter;
use rpc_introspection::schema::{
    OneOfVariant, RpcObjectField, RpcObjectType, RpcSchema, RpcTypeSchema,
};
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

            let projection = entity_type
                .projection_by_name(projection_name)
                .unwrap_or_else(|| {
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

/// Build a return type schema that includes all projections as a oneOf.
/// Generates all projection types and wraps them in a oneOf at the innermost level,
/// preserving the outer Optional/List wrapping.
pub(crate) fn build_return_type_schema_all_projections(
    return_type: &OperationReturnType<EntityType>,
    subsystem: &PostgresRpcSubsystemWithRouter,
    schema: &mut RpcSchema,
    added_types: &mut HashSet<String>,
) -> RpcTypeSchema {
    match return_type {
        OperationReturnType::Plain(base) => {
            let entity_type = &subsystem.core_subsystem.entity_types[base.associated_type_id];

            let variants: Vec<OneOfVariant> = entity_type
                .projections
                .iter()
                .map(|projection| {
                    let type_name = projection_type_name(&entity_type.name, &projection.name);
                    ensure_projection_type_added(
                        &type_name,
                        entity_type,
                        projection,
                        subsystem,
                        schema,
                        added_types,
                    );
                    OneOfVariant::Ref(type_name)
                })
                .collect();

            RpcTypeSchema::one_of(variants)
        }
        OperationReturnType::Optional(inner) => RpcTypeSchema::optional(
            build_return_type_schema_all_projections(inner, subsystem, schema, added_types),
        ),
        OperationReturnType::List(inner) => RpcTypeSchema::array(
            build_return_type_schema_all_projections(inner, subsystem, schema, added_types),
        ),
    }
}

/// Build the return type schema for an entity as a oneOf of all its projection variants.
/// Every entity has at least `pk` and `basic` projections.
pub(crate) fn build_return_type_schema_for_entity(
    return_type: &OperationReturnType<EntityType>,
    subsystem: &PostgresRpcSubsystemWithRouter,
    schema: &mut RpcSchema,
    added_types: &mut HashSet<String>,
) -> RpcTypeSchema {
    build_return_type_schema_all_projections(return_type, subsystem, schema, added_types)
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
        name => format!("{entity_name}{}", capitalize(name)),
    }
}

/// Generate a type name for a relation projected with one or more projections.
/// For a single name, delegates to `projection_type_name`.
/// For multiple, filters out "pk" (always a subset), sorts the rest, and combines.
fn relation_projection_type_name(entity_name: &str, projection_names: &[String]) -> String {
    if projection_names.len() == 1 {
        return projection_type_name(entity_name, &projection_names[0]);
    }

    // Filter out pk — it is always a subset of any other projection
    let custom_names: Vec<&str> = projection_names
        .iter()
        .map(|s| s.as_str())
        .filter(|n| *n != PROJECTION_PK)
        .collect();

    match custom_names.len() {
        0 => pk_type_name(entity_name),
        1 => projection_type_name(entity_name, custom_names[0]),
        _ => {
            // TODO: Move this to builder (then we can use heck to do proper CamelCase) — we want to avoid doing this string manipulation in the common case of a single projection
            let mut capitalized: Vec<String> =
                custom_names.iter().map(|name| capitalize(name)).collect();
            capitalized.sort();
            format!("{entity_name}{}", capitalized.join(""))
        }
    }
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) => c.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// Create a merged projection by unioning fields from multiple named projections.
fn merge_projections(entity_type: &EntityType, projection_names: &[String]) -> ResolvedProjection {
    // Fast path: single projection doesn't need merging
    if projection_names.len() == 1 {
        let proj = entity_type
            .projection_by_name(&projection_names[0])
            .unwrap_or_else(|| {
                panic!(
                    "Projection `{}` not found for type `{}`",
                    projection_names[0], entity_type.name
                )
            });
        return proj.clone();
    }

    let mut scalar_elements: Vec<ProjectionElement> = Vec::new();
    let mut seen_scalars: HashSet<String> = HashSet::new();
    let mut relation_entries: IndexMap<String, HashSet<String>> = IndexMap::new();

    for proj_name in projection_names {
        let projection = entity_type
            .projection_by_name(proj_name)
            .unwrap_or_else(|| {
                panic!(
                    "Projection `{}` not found for type `{}`",
                    proj_name, entity_type.name
                )
            });

        for element in &projection.elements {
            match element {
                ProjectionElement::ScalarField(name) => {
                    if seen_scalars.insert(name.clone()) {
                        scalar_elements.push(element.clone());
                    }
                }
                ProjectionElement::RelationProjection {
                    relation_field_name,
                    projection_names: nested,
                } => {
                    relation_entries
                        .entry(relation_field_name.clone())
                        .or_default()
                        .extend(nested.iter().cloned());
                }
            }
        }
    }

    let mut merged_elements = scalar_elements;
    for (relation_name, names) in relation_entries {
        merged_elements.push(ProjectionElement::RelationProjection {
            relation_field_name: relation_name,
            projection_names: names.into_iter().collect(),
        });
    }

    ResolvedProjection {
        name: projection_names.join("_"),
        elements: merged_elements,
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
                projection_names,
            } => {
                if let Some(field) = entity_type.field_by_name(relation_field_name) {
                    let mut get_foreign_type_name = |foreign_entity: &EntityType| -> String {
                        let rel_type_name =
                            relation_projection_type_name(&foreign_entity.name, projection_names);
                        if !added_types.contains(&rel_type_name) {
                            let merged = merge_projections(foreign_entity, projection_names);
                            ensure_projection_type_added(
                                &rel_type_name,
                                foreign_entity,
                                &merged,
                                subsystem,
                                schema,
                                added_types,
                            );
                        }
                        rel_type_name
                    };

                    match &field.relation {
                        PostgresRelation::ManyToOne { relation, .. } => {
                            let foreign_entity =
                                &subsystem.core_subsystem.entity_types[relation.foreign_entity_id];
                            let rel_type_name = get_foreign_type_name(foreign_entity);

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
                            let rel_type_name = get_foreign_type_name(foreign_entity);

                            let array_schema =
                                RpcTypeSchema::array(RpcTypeSchema::object(&rel_type_name));

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
