// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::collections::HashMap;

use crate::access_builder::ResolvedAccess;
use crate::resolved_type::{
    ResolvedCompositeType, ResolvedField, ResolvedFieldDefault, ResolvedFieldType,
    ResolvedFieldTypeHelper, ResolvedType, ResolvedTypeEnv, ResolvedTypeHint,
};
use core_plugin_interface::core_model::access::AccessPredicateExpression;
use postgres_core_model::types::EntityRepresentation;

use crate::{aggregate_type_builder::aggregate_type_name, shallow::Shallow};

use super::access_utils;

use core_plugin_interface::{
    core_model::{
        mapped_arena::SerializableSlabIndex,
        primitive_type::PrimitiveType,
        types::{FieldType, Named, TypeValidationProvider},
    },
    core_model_builder::{ast::ast_types::AstExpr, error::ModelBuildingError, typechecker::Typed},
};

use exo_sql::{ColumnId, VectorDistanceFunction, DEFAULT_VECTOR_SIZE};

use postgres_core_model::{
    access::{
        Access, DatabaseAccessPrimitiveExpression, InputAccessPrimitiveExpression,
        UpdateAccessExpression,
    },
    aggregate::{AggregateField, AggregateFieldType},
    relation::{ManyToOneRelation, OneToManyRelation, PostgresRelation, RelationCardinality},
    types::{
        get_field_id, EntityType, PostgresField, PostgresFieldType, PostgresPrimitiveType,
        TypeIndex,
    },
    vector_distance::{VectorDistanceField, VectorDistanceType},
};

use super::system_builder::SystemContextBuilding;

pub(super) fn build_shallow(resolved_env: &ResolvedTypeEnv, building: &mut SystemContextBuilding) {
    for (_, resolved_type) in resolved_env.resolved_types.iter() {
        create_shallow_type(resolved_type, resolved_env, building);
    }
}

pub(super) fn build_expanded(
    resolved_env: &ResolvedTypeEnv,
    building: &mut SystemContextBuilding,
) -> Result<(), ModelBuildingError> {
    for (_, resolved_type) in resolved_env.resolved_types.iter() {
        if let ResolvedType::Composite(c) = &resolved_type {
            associate_datbase_table(c, building);
        }
    }

    for (_, resolved_type) in resolved_env.resolved_types.iter() {
        if let ResolvedType::Composite(c) = &resolved_type {
            expand_type_fields(c, building, resolved_env, false)?;
        }
    }

    for (_, resolved_type) in resolved_env.resolved_types.iter() {
        if let ResolvedType::Composite(c) = &resolved_type {
            expand_type_fields(c, building, resolved_env, true)?;
        }
    }

    for (_, resolved_type) in resolved_env.resolved_types.iter() {
        if let ResolvedType::Composite(c) = &resolved_type {
            expand_dynamic_default_values(c, building, resolved_env)?;
        }
    }

    for (_, resolved_type) in resolved_env.resolved_types.iter() {
        if let ResolvedType::Composite(c) = &resolved_type {
            expand_type_access(c, resolved_env, building)?;
        }
    }

    Ok(())
}

fn create_shallow_type(
    resolved_type: &ResolvedType,
    _resolved_env: &ResolvedTypeEnv,
    building: &mut SystemContextBuilding,
) {
    match resolved_type {
        ResolvedType::Primitive(pt) => {
            building.primitive_types.add(
                &resolved_type.name(),
                PostgresPrimitiveType {
                    name: resolved_type.name(),
                },
            );
            if matches!(pt, PrimitiveType::Vector) {
                let vector_distance_type = VectorDistanceType::new("VectorDistance".to_string());
                building
                    .vector_distance_types
                    .add("VectorDistance", vector_distance_type);
            }
        }
        ResolvedType::Composite(composite) => {
            let typ = EntityType {
                name: resolved_type.name(),
                plural_name: resolved_type.plural_name(),
                representation: composite.representation,
                fields: vec![],
                agg_fields: vec![],
                vector_distance_fields: vec![],
                table_id: SerializableSlabIndex::shallow(),
                access: restrictive_access(),
            };

            building.entity_types.add(&resolved_type.name(), typ);
        }
    }
}

/// Expand a composite type except for creating its fields.
///
/// Specifically: Create and set the table along with its columns. However, columns will not have its references set

///
/// This allows the type to become `Composite` and `table_id` for any type can be accessed when building fields in the next step of expansion.
/// We can't expand fields yet since creating a field requires access to columns (self as well as those in a referred field in case a relation)
/// and we may not have expanded a referred type yet.
fn associate_datbase_table(
    resolved_type: &ResolvedCompositeType,
    building: &mut SystemContextBuilding,
) {
    if resolved_type.representation == EntityRepresentation::Json {
        return;
    }

    let existing_type_id = building.get_entity_type_id(&resolved_type.name).unwrap();
    let existing_type = &mut building.entity_types[existing_type_id];
    existing_type.table_id = building
        .database
        .get_table_id(&resolved_type.table_name)
        .unwrap();
}

/// Now that all types have table with them (set in the earlier associate_datbase_table phase), we can
/// expand fields
fn expand_type_fields(
    resolved_type: &ResolvedCompositeType,
    building: &mut SystemContextBuilding,
    resolved_env: &ResolvedTypeEnv,
    expand_relations: bool,
) -> Result<(), ModelBuildingError> {
    let existing_type_id = building.get_entity_type_id(&resolved_type.name).unwrap();

    let entity_fields: Result<Vec<_>, _> = resolved_type
        .fields
        .iter()
        .map(|field| {
            create_persistent_field(
                field,
                &existing_type_id,
                building,
                resolved_env,
                expand_relations,
            )
        })
        .collect();
    let entity_fields = entity_fields?;

    let agg_fields = resolved_type
        .fields
        .iter()
        .flat_map(|field| {
            create_agg_field(
                field,
                &existing_type_id,
                building,
                resolved_env,
                expand_relations,
            )
        })
        .collect();

    let vector_distance_fields = resolved_type
        .fields
        .iter()
        .flat_map(|field| {
            create_vector_distance_field(field, &existing_type_id, building, resolved_env)
        })
        .collect();

    let existing_type = &mut building.entity_types[existing_type_id];
    existing_type.fields = entity_fields;
    existing_type.agg_fields = agg_fields;
    existing_type.vector_distance_fields = vector_distance_fields;

    Ok(())
}

// Expand dynamic default values (pre-condition: all type fields have been populated)
fn expand_dynamic_default_values(
    resolved_type: &ResolvedCompositeType,
    building: &mut SystemContextBuilding,
    resolved_env: &ResolvedTypeEnv,
) -> Result<(), ModelBuildingError> {
    fn matches(
        field_type: &FieldType<PostgresFieldType<EntityType>>,
        context_type: &FieldType<PrimitiveType>,
    ) -> bool {
        match (field_type, context_type) {
            (FieldType::Plain(field_type), FieldType::Plain(context_type)) => {
                field_type.name() == context_type.name()
            }
            (FieldType::List(field_type), FieldType::List(context_type)) => {
                matches(field_type.as_ref(), context_type.as_ref())
            }
            (FieldType::Optional(field_type), FieldType::Optional(context_type)) => {
                matches(field_type.as_ref(), context_type.as_ref())
            }
            _ => false,
        }
    }

    let existing_type_id = building.get_entity_type_id(&resolved_type.name).unwrap();

    let dynamic_default_values = {
        let existing_type = &building.entity_types[existing_type_id];

        resolved_type
            .fields
            .iter()
            .flat_map(|resolved_field| {
                let entity_field = existing_type
                    .fields
                    .iter()
                    .find(|field| field.name == resolved_field.name)
                    .unwrap();

                let dynamic_default_value = match resolved_field.default_value.as_ref() {
                    Some(ResolvedFieldDefault::Value(expr)) => match expr.as_ref() {
                        AstExpr::FieldSelection(selection) => {
                            let (context_selection, context_type) = selection.get_context(
                                resolved_env.contexts,
                                resolved_env.function_definitions,
                            )?;

                            match entity_field.relation {
                                PostgresRelation::Scalar { .. } => {
                                    let field_type = &entity_field.typ;
                                    if !matches(field_type, context_type) {
                                        Err(ModelBuildingError::Generic(
                                            "Type of default value does not match field type"
                                                .to_string(),
                                        ))
                                    } else {
                                        Ok(Some(context_selection))
                                    }
                                }
                                PostgresRelation::ManyToOne(ManyToOneRelation {
                                    foreign_pk_field_id: foreign_field_id,
                                    ..
                                }) => {
                                    let foreign_type_pk = &foreign_field_id
                                        .resolve(building.entity_types.values_ref())
                                        .typ;

                                    if !matches(foreign_type_pk, context_type) {
                                        Err(ModelBuildingError::Generic(
                                            "Type of default value does not match field type"
                                                .to_string(),
                                        ))
                                    } else {
                                        Ok(Some(context_selection))
                                    }
                                }
                                _ => Err(ModelBuildingError::Generic(
                                    "Invalid relation type for default value".to_string(),
                                )),
                            }
                        }
                        _ => Ok(None),
                    },
                    _ => Ok(None),
                };

                dynamic_default_value.map(|value| (resolved_field.name.clone(), value))
            })
            .collect::<Vec<_>>()
    };

    dynamic_default_values
        .into_iter()
        .for_each(|(field_name, value)| {
            let existing_type = &mut building.entity_types[existing_type_id];
            let existing_field = existing_type
                .fields
                .iter_mut()
                .find(|field| field.name == field_name)
                .unwrap();
            existing_field.dynamic_default_value = value;
        });

    Ok(())
}

// Expand access expressions (pre-condition: all type fields have been populated)
fn expand_type_access(
    resolved_type: &ResolvedCompositeType,
    resolved_env: &ResolvedTypeEnv,
    building: &mut SystemContextBuilding,
) -> Result<(), ModelBuildingError> {
    let existing_type_id = building.get_entity_type_id(&resolved_type.name).unwrap();

    let expr = compute_access(
        &resolved_type.access,
        existing_type_id,
        resolved_env,
        building,
    )?;

    let existing_type = &mut building.entity_types[existing_type_id];

    existing_type.access = expr;

    Ok(())
}

/// Compute access expression for database access.
///
/// Goes over the chain of the expressions and maps the first non-optional expression to a database access expression.
fn compute_database_access_expr(
    ast_exprs: &[&Option<AstExpr<Typed>>],
    entity_id: SerializableSlabIndex<EntityType>,
    resolved_env: &ResolvedTypeEnv,
    building: &SystemContextBuilding,
) -> Result<
    SerializableSlabIndex<AccessPredicateExpression<DatabaseAccessPrimitiveExpression>>,
    ModelBuildingError,
> {
    let entity = &building.entity_types[entity_id];

    let expr = ast_exprs
        .iter()
        .find_map(|ast_expr| {
            ast_expr.as_ref().map(|ast_expr| {
                access_utils::compute_predicate_expression(
                    ast_expr,
                    entity,
                    HashMap::new(),
                    resolved_env,
                    &building.primitive_types,
                    &building.entity_types,
                    &building.database,
                )
            })
        })
        .transpose()?;

    Ok(match expr {
        Some(AccessPredicateExpression::BooleanLiteral(false)) | None => building
            .database_access_expressions
            .lock()
            .unwrap()
            .restricted_access_index(),
        Some(expr) => building
            .database_access_expressions
            .lock()
            .unwrap()
            .insert(expr),
    })
}

fn compute_input_access_expr(
    ast_exprs: &[&Option<AstExpr<Typed>>],
    entity_id: SerializableSlabIndex<EntityType>,
    resolved_env: &ResolvedTypeEnv,
    building: &SystemContextBuilding,
) -> Result<
    SerializableSlabIndex<AccessPredicateExpression<InputAccessPrimitiveExpression>>,
    ModelBuildingError,
> {
    let entity = &building.entity_types[entity_id];

    let expr = ast_exprs
        .iter()
        .find_map(|ast_expr| {
            ast_expr.as_ref().map(|ast_expr| {
                access_utils::compute_input_predicate_expression(
                    ast_expr,
                    HashMap::from_iter([("self".to_string(), entity)]),
                    resolved_env,
                    &building.primitive_types,
                    &building.entity_types,
                )
            })
        })
        .transpose()?;

    Ok(match expr {
        Some(AccessPredicateExpression::BooleanLiteral(false)) | None => building
            .input_access_expressions
            .lock()
            .unwrap()
            .restricted_access_index(),
        Some(expr) => building
            .input_access_expressions
            .lock()
            .unwrap()
            .insert(expr),
    })
}

fn compute_access(
    resolved: &ResolvedAccess,
    entity_id: SerializableSlabIndex<EntityType>,
    resolved_env: &ResolvedTypeEnv,
    building: &SystemContextBuilding,
) -> Result<Access, ModelBuildingError> {
    let compute_input_access_expr = |ast_exprs: &[&Option<AstExpr<Typed>>]| {
        compute_input_access_expr(ast_exprs, entity_id, resolved_env, building)
    };

    let creation_input_access =
        compute_input_access_expr(&[&resolved.creation, &resolved.mutation, &resolved.default])?;
    let update_input_access =
        compute_input_access_expr(&[&resolved.update, &resolved.mutation, &resolved.default])?;

    let compute_database_access_expr = |ast_exprs: &[&Option<AstExpr<Typed>>]| {
        compute_database_access_expr(ast_exprs, entity_id, resolved_env, building)
    };

    let query_access = compute_database_access_expr(&[&resolved.query, &resolved.default])?;
    let update_database_access =
        compute_database_access_expr(&[&resolved.update, &resolved.mutation, &resolved.default])?;
    let delete_access =
        compute_database_access_expr(&[&resolved.delete, &resolved.mutation, &resolved.default])?;

    Ok(Access {
        read: query_access,
        creation: creation_input_access,
        update: UpdateAccessExpression {
            input: update_input_access,
            database: update_database_access,
        },
        delete: delete_access,
    })
}

fn create_persistent_field(
    field: &ResolvedField,
    type_id: &SerializableSlabIndex<EntityType>,
    building: &SystemContextBuilding,
    env: &ResolvedTypeEnv,
    expand_foreign_relations: bool,
) -> Result<PostgresField<EntityType>, ModelBuildingError> {
    let base_field_type = {
        let ResolvedFieldType {
            type_name,
            is_primitive,
        } = field.typ.innermost();

        if *is_primitive {
            let type_id = building.primitive_types.get_id(type_name).unwrap();
            PostgresFieldType {
                type_name: type_name.clone(),
                type_id: TypeIndex::Primitive(type_id),
            }
        } else {
            let type_id = building.entity_types.get_id(type_name).unwrap();
            PostgresFieldType {
                type_name: type_name.clone(),
                type_id: TypeIndex::Composite(type_id),
            }
        }
    };

    let relation = create_relation(field, *type_id, building, env, expand_foreign_relations);

    let access = compute_access(&field.access, *type_id, env, building)?;

    let type_validation = match &field.type_hint {
        Some(th) => th.get_type_validation(),
        None => None,
    };

    Ok(PostgresField {
        name: field.name.to_owned(),
        typ: field.typ.wrap(base_field_type),
        relation,
        access,
        has_default_value: field.default_value.is_some(),
        dynamic_default_value: None,
        readonly: field.readonly || field.update_sync,
        type_validation,
    })
}

fn create_agg_field(
    field: &ResolvedField,
    type_id: &SerializableSlabIndex<EntityType>,
    building: &SystemContextBuilding,
    env: &ResolvedTypeEnv,
    expand_foreign_relations: bool,
) -> Option<AggregateField> {
    fn is_underlying_type_list(field_type: &FieldType<ResolvedFieldType>) -> bool {
        match field_type {
            FieldType::Plain(_) => false,
            FieldType::Optional(underlying) => is_underlying_type_list(underlying),
            FieldType::List(_) => true,
        }
    }

    if field.typ.innermost().is_primitive || !is_underlying_type_list(&field.typ) {
        None
    } else {
        let field_name = format!("{}Agg", field.name);
        let field_type_name = field.typ.name();
        let agg_type_name = aggregate_type_name(field_type_name);
        let agg_type_id = building.aggregate_types.get_id(&agg_type_name).unwrap();

        let relation = Some(create_relation(
            field,
            *type_id,
            building,
            env,
            expand_foreign_relations,
        ));

        Some(AggregateField {
            name: field_name,
            typ: AggregateFieldType::Composite {
                type_name: agg_type_name,
                type_id: agg_type_id,
            },
            relation,
        })
    }
}

fn create_vector_distance_field(
    field: &ResolvedField,
    type_id: &SerializableSlabIndex<EntityType>,
    building: &SystemContextBuilding,
    env: &ResolvedTypeEnv,
) -> Option<VectorDistanceField> {
    match field.type_hint {
        Some(ResolvedTypeHint::Vector {
            size,
            distance_function,
        }) => {
            let self_type = &building.entity_types[*type_id];
            let self_table_id = &self_type.table_id;
            let column_id = building
                .database
                .get_column_id(*self_table_id, &field.column_name)
                .unwrap();

            let access = compute_access(&field.access, *type_id, env, building).unwrap();

            Some(VectorDistanceField {
                name: format!("{}Distance", field.name),
                column_id,
                size: size.unwrap_or(DEFAULT_VECTOR_SIZE),
                distance_function: distance_function.unwrap_or(VectorDistanceFunction::default()),
                access,
            })
        }
        _ => None,
    }
}

fn create_relation(
    field: &ResolvedField,
    type_id: SerializableSlabIndex<EntityType>,
    building: &SystemContextBuilding,
    resolved_env: &ResolvedTypeEnv,
    expand_foreign_relations: bool,
) -> PostgresRelation {
    fn placeholder_relation() -> PostgresRelation {
        // Create an impossible value (will be filled later when expanding relations)
        PostgresRelation::Scalar {
            column_id: ColumnId {
                table_id: SerializableSlabIndex::from_idx(usize::MAX),
                column_index: usize::MAX,
            },
        }
    }

    let self_type = &building.entity_types[type_id];
    let self_table_id = &self_type.table_id;

    if field.is_pk {
        let column_id = building
            .database
            .get_column_id(*self_table_id, &field.column_name)
            .unwrap();
        PostgresRelation::Pk { column_id }
    } else {
        // we can treat Optional fields as their inner type for the purposes of computing relations
        let field_base_typ = &field.typ.base_type();

        match field_base_typ {
            FieldType::List(underlying) => {
                if self_type.representation == EntityRepresentation::Json {
                    PostgresRelation::Embedded
                } else {
                    // Since the field type is a list, the relation depends on the underlying type.
                    // 1. If it is a primitive, we treat it as a scalar ("List" of a primitive type is still a scalar from the database perspective)
                    // 2. Otherwise (if it is a composite), it is a one-to-many relation.
                    match underlying.deref(resolved_env) {
                        ResolvedType::Primitive(_) => PostgresRelation::Scalar {
                            column_id: building
                                .database
                                .get_column_id(*self_table_id, &field.column_name)
                                .unwrap(),
                        },
                        ResolvedType::Composite(foreign_field_type) => {
                            if foreign_field_type.representation == EntityRepresentation::Json {
                                PostgresRelation::Scalar {
                                    column_id: building
                                        .database
                                        .get_column_id(*self_table_id, &field.column_name)
                                        .unwrap(),
                                }
                            } else if expand_foreign_relations {
                                compute_many_to_one(
                                    field,
                                    foreign_field_type,
                                    RelationCardinality::Unbounded,
                                    building,
                                )
                            } else {
                                placeholder_relation()
                            }
                        }
                    }
                }
            }

            FieldType::Plain(ResolvedFieldType { type_name, .. }) => {
                let foreign_resolved_type = resolved_env.get_by_key(type_name).unwrap();

                match foreign_resolved_type {
                    ResolvedType::Primitive(_) => {
                        if self_type.representation == EntityRepresentation::Json {
                            PostgresRelation::Embedded
                        } else {
                            let column_id = building
                                .database
                                .get_column_id(*self_table_id, &field.column_name)
                                .unwrap();
                            PostgresRelation::Scalar { column_id }
                        }
                    }
                    ResolvedType::Composite(foreign_field_type) => {
                        if foreign_field_type.representation == EntityRepresentation::Json {
                            PostgresRelation::Scalar {
                                column_id: building
                                    .database
                                    .get_column_id(*self_table_id, &field.column_name)
                                    .unwrap(),
                            }
                        } else {
                            // A field's type is "Plain" or "Optional" and the field type is composite,
                            // but we can't be sure if this is a ManyToOne or OneToMany unless we examine the other side's type.
                            let foreign_type_field_typ = &foreign_resolved_type
                                .as_composite()
                                .field_by_column_name(&field.column_name)
                                .unwrap()
                                .typ;

                            match (&field.typ, foreign_type_field_typ) {
                                (FieldType::Optional(_), FieldType::Plain(_)) => {
                                    if expand_foreign_relations {
                                        compute_many_to_one(
                                            field,
                                            foreign_field_type,
                                            RelationCardinality::Optional,
                                            building,
                                        )
                                    } else {
                                        placeholder_relation()
                                    }
                                }
                                (FieldType::Plain(_), FieldType::Optional(_)) => {
                                    if expand_foreign_relations {
                                        compute_one_to_many_relation(
                                            field,
                                            self_type,
                                            foreign_field_type,
                                            RelationCardinality::Optional,
                                            building,
                                        )
                                    } else {
                                        placeholder_relation()
                                    }
                                }
                                (field_typ, foreign_type_field_typ) => {
                                    match (field_base_typ, foreign_type_field_typ.base_type()) {
                                        (FieldType::Plain(_), FieldType::List(_)) => {
                                            if expand_foreign_relations {
                                                compute_one_to_many_relation(
                                                    field,
                                                    self_type,
                                                    foreign_field_type,
                                                    RelationCardinality::Unbounded,
                                                    building,
                                                )
                                            } else {
                                                placeholder_relation()
                                            }
                                        }
                                        _ => {
                                            panic!(
                                            "Unexpected relation type for field `{}` of {:?} type. The matching field is {:?}",
                                            field.name, field_typ, foreign_field_type
                                        )
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            FieldType::Optional(_) => panic!("Optional in an Optional?"),
        }
    }
}

fn compute_many_to_one(
    field: &ResolvedField,
    foreign_field_type: &ResolvedCompositeType,
    cardinality: RelationCardinality,
    building: &SystemContextBuilding,
) -> PostgresRelation {
    // If the field is of a list type and the underlying type is not a primitive,
    // then it is a OneToMany relation with the self's type being the "One" side
    // and the field's type being the "Many" side.
    let foreign_type_id = building
        .get_entity_type_id(&foreign_field_type.name)
        .unwrap();
    let foreign_type = &building.entity_types[foreign_type_id];
    let foreign_table_id = foreign_type.table_id;

    let foreign_column_id = building
        .database
        .get_column_id(foreign_table_id, &field.column_name)
        .unwrap();

    let foreign_resolved_field = foreign_field_type
        .fields
        .iter()
        .find(|f| f.column_name == field.column_name)
        .unwrap();

    let foreign_field_id = get_field_id(
        building.entity_types.values_ref(),
        foreign_type_id,
        &foreign_resolved_field.name,
    )
    .unwrap();

    let relation_id = foreign_column_id
        .get_otm_relation(&building.database)
        .unwrap();

    PostgresRelation::OneToMany(OneToManyRelation {
        foreign_field_id,
        cardinality,
        relation_id,
    })
}

fn compute_one_to_many_relation(
    field: &ResolvedField,
    self_type: &EntityType,
    foreign_field_type: &ResolvedCompositeType,
    cardinality: RelationCardinality,
    building: &SystemContextBuilding,
) -> PostgresRelation {
    let self_table_id = &self_type.table_id;

    let foreign_type_id = building
        .get_entity_type_id(&foreign_field_type.name)
        .unwrap();
    let foreign_type = &building.entity_types[foreign_type_id];

    let self_column_id = building
        .database
        .get_column_id(*self_table_id, &field.column_name)
        .unwrap();
    let foreign_pk_field_id = foreign_type.pk_field_id(foreign_type_id).unwrap();

    let relation_id = self_column_id.get_mto_relation(&building.database).unwrap();

    PostgresRelation::ManyToOne(ManyToOneRelation {
        cardinality,
        foreign_pk_field_id,
        relation_id,
    })
}

fn restrictive_access() -> Access {
    Access {
        creation: SerializableSlabIndex::shallow(),
        read: SerializableSlabIndex::shallow(),
        update: UpdateAccessExpression {
            input: SerializableSlabIndex::shallow(),
            database: SerializableSlabIndex::shallow(),
        },
        delete: SerializableSlabIndex::shallow(),
    }
}