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
use crate::resolved_builder::Cardinality;
use crate::{
    resolved_type::{
        ResolvedCompositeType, ResolvedField, ResolvedFieldDefault, ResolvedFieldType,
        ResolvedFieldTypeHelper, ResolvedType, ResolvedTypeEnv,
    },
    type_provider::VectorTypeHint,
};
use common::value::Val;
use common::value::val::ValNumber;
use core_model::access::AccessPredicateExpression;
use core_model::primitive_type;
use core_model::types::{Named, TypeValidationProvider};
use postgres_core_model::access::{CreationAccessExpression, PrecheckAccessPrimitiveExpression};
use postgres_core_model::types::{
    EntityRepresentation, PostgresFieldDefaultValue, PostgresPrimitiveTypeKind,
};

use crate::{aggregate_type_builder::aggregate_type_name, shallow::Shallow};

use super::access;

use core_model::{
    mapped_arena::SerializableSlabIndex, primitive_type::PrimitiveType, types::FieldType,
};
use core_model_builder::{ast::ast_types::AstExpr, error::ModelBuildingError, typechecker::Typed};

use exo_sql::{
    ColumnId, DEFAULT_VECTOR_SIZE, VectorDistanceFunction, get_mto_relation_for_columns,
    get_otm_relation_for_columns,
};

use postgres_core_model::{
    access::{Access, DatabaseAccessPrimitiveExpression, UpdateAccessExpression},
    aggregate::{AggregateField, AggregateFieldType},
    relation::{ManyToOneRelation, OneToManyRelation, PostgresRelation, RelationCardinality},
    types::{EntityType, PostgresField, PostgresFieldType, PostgresPrimitiveType, TypeIndex},
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
                    name: resolved_type.name().to_owned(),
                    kind: PostgresPrimitiveTypeKind::Builtin,
                },
            );
            if matches!(pt, PrimitiveType::Plain(pbt) if pbt.name() == primitive_type::VectorType::NAME)
            {
                let vector_distance_type = VectorDistanceType::new("VectorDistance".to_string());
                building
                    .vector_distance_types
                    .add("VectorDistance", vector_distance_type);
            }
        }
        ResolvedType::Enum(enum_type) => {
            building.primitive_types.add(
                &enum_type.name,
                PostgresPrimitiveType {
                    name: enum_type.name.clone(),
                    kind: PostgresPrimitiveTypeKind::Enum(enum_type.fields.to_vec()),
                },
            );
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
                doc_comments: composite.doc_comments.clone(),
            };

            building.entity_types.add(&resolved_type.name(), typ);
        }
    }
}

/// Expand a composite type except for creating its fields.
///
/// Specifically: Create and set the table along with its columns. However, columns will not have its references set.
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
        .map(|field| {
            create_agg_field(
                field,
                &existing_type_id,
                building,
                resolved_env,
                expand_relations,
            )
        })
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .flatten()
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
            (FieldType::Optional(field_type), FieldType::Plain(context_type)) => {
                field_type.name() == context_type.name()
            }
            _ => false,
        }
    }

    let existing_type_id = building.get_entity_type_id(&resolved_type.name).unwrap();

    let default_values = {
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

                            match &entity_field.relation {
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
                                PostgresRelation::ManyToOne {
                                    relation:
                                        ManyToOneRelation {
                                            foreign_pk_field_ids,
                                            ..
                                        },
                                    ..
                                } => {
                                    if foreign_pk_field_ids.len() != 1 {
                                        return Err(ModelBuildingError::Generic(
                                            "Context-based initialization of composite pk fields is not supported".to_string(),
                                        ));
                                    }
                                    let foreign_type_pk = &foreign_pk_field_ids[0]
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

    default_values.into_iter().for_each(|(field_name, value)| {
        let existing_type = &mut building.entity_types[existing_type_id];
        let existing_field = existing_type
            .fields
            .iter_mut()
            .find(|field| field.name == field_name)
            .unwrap();
        if let Some(value) = value {
            existing_field.default_value = Some(PostgresFieldDefaultValue::Dynamic(value));
        }
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

    {
        let existing_type = &mut building.entity_types[existing_type_id];

        existing_type.access = expr;
    }

    for field in resolved_type.fields.iter() {
        let expr = compute_access(&field.access, existing_type_id, resolved_env, building)?;

        let existing_type = &mut building.entity_types[existing_type_id];

        existing_type
            .fields
            .iter_mut()
            .find(|f| f.name == field.name)
            .unwrap()
            .access = expr;
    }

    Ok(())
}

fn first_non_optional_access_expr<'a>(
    ast_exprs: &[&'a Option<AstExpr<Typed>>],
) -> Option<&'a AstExpr<Typed>> {
    ast_exprs.iter().copied().flatten().next()
}

/// Compute access expression for database access.
///
/// Goes over the chain of the expressions and maps the first non-optional expression to a database access expression.
/// If no non-optional expression is found, returns a restricted access expression.
fn compute_database_access_expr(
    ast_exprs: &[&Option<AstExpr<Typed>>],
    entity_id: SerializableSlabIndex<EntityType>,
    resolved_env: &ResolvedTypeEnv,
    building: &SystemContextBuilding,
) -> Result<
    SerializableSlabIndex<AccessPredicateExpression<DatabaseAccessPrimitiveExpression>>,
    ModelBuildingError,
> {
    let ast_expr = first_non_optional_access_expr(ast_exprs);

    let restricted_access_index = || {
        building
            .database_access_expressions
            .lock()
            .unwrap()
            .restricted_access_index()
    };

    let access_predicate_expr = match ast_expr {
        Some(ast_expr) => {
            let entity = &building.entity_types[entity_id];

            access::compute_predicate_expression(
                ast_expr,
                entity,
                HashMap::new(),
                resolved_env,
                &building.primitive_types,
                &building.entity_types,
                &building.database,
            )
        }
        None => return Ok(restricted_access_index()),
    }?;

    Ok(match access_predicate_expr {
        AccessPredicateExpression::BooleanLiteral(false) => restricted_access_index(),
        _ => building
            .database_access_expressions
            .lock()
            .unwrap()
            .insert(access_predicate_expr),
    })
}

fn compute_precheck_access_expr(
    ast_exprs: &[&Option<AstExpr<Typed>>],
    entity_id: SerializableSlabIndex<EntityType>,
    resolved_env: &ResolvedTypeEnv,
    building: &SystemContextBuilding,
) -> Result<
    SerializableSlabIndex<AccessPredicateExpression<PrecheckAccessPrimitiveExpression>>,
    ModelBuildingError,
> {
    let entity = &building.entity_types[entity_id];

    let ast_expr = first_non_optional_access_expr(ast_exprs);

    let restricted_access_index = || {
        building
            .precheck_access_expressions
            .lock()
            .unwrap()
            .restricted_access_index()
    };

    let expr = match ast_expr {
        Some(ast_expr) => access::compute_precheck_predicate_expression(
            ast_expr,
            entity,
            HashMap::new(),
            resolved_env,
            &building.primitive_types,
            &building.entity_types,
            &building.database,
        ),
        _ => return Ok(restricted_access_index()),
    }?;

    Ok(match expr {
        AccessPredicateExpression::BooleanLiteral(false) => restricted_access_index(),
        _ => building
            .precheck_access_expressions
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
    let compute_database_access_expr = |ast_exprs: &[&Option<AstExpr<Typed>>]| {
        compute_database_access_expr(ast_exprs, entity_id, resolved_env, building)
    };

    let query_access = compute_database_access_expr(&[&resolved.query, &resolved.default])?;
    let creation_precheck_access = compute_precheck_access_expr(
        &[&resolved.creation, &resolved.mutation, &resolved.default],
        entity_id,
        resolved_env,
        building,
    )?;

    let update_precheck_access = compute_precheck_access_expr(
        &[&resolved.update, &resolved.mutation, &resolved.default],
        entity_id,
        resolved_env,
        building,
    )?;
    let update_database_access =
        compute_database_access_expr(&[&resolved.update, &resolved.mutation, &resolved.default])?;
    let delete_access =
        compute_database_access_expr(&[&resolved.delete, &resolved.mutation, &resolved.default])?;

    Ok(Access {
        read: query_access,
        creation: CreationAccessExpression {
            precheck: creation_precheck_access,
        },
        update: UpdateAccessExpression {
            precheck: update_precheck_access,
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
    let base_field_type =
        {
            let ResolvedFieldType {
                type_name,
                is_primitive,
            } = field.typ.innermost();

            if *is_primitive {
                let type_id = building.primitive_types.get_id(type_name).ok_or(
                    ModelBuildingError::Generic(format!(
                        "Primitive type `{}` not found",
                        type_name
                    )),
                )?;
                Ok::<_, ModelBuildingError>(PostgresFieldType {
                    type_name: type_name.to_owned(),
                    type_id: TypeIndex::Primitive(type_id),
                })
            } else {
                let type_id =
                    building
                        .entity_types
                        .get_id(type_name)
                        .ok_or(ModelBuildingError::Generic(format!(
                            "Composite type `{}` not found",
                            type_name
                        )))?;
                Ok::<_, ModelBuildingError>(PostgresFieldType {
                    type_name: type_name.to_owned(),
                    type_id: TypeIndex::Composite(type_id),
                })
            }
        }?;

    let relation = create_relation(field, *type_id, building, env, expand_foreign_relations)?;

    // Use shallow access expressions for fields at this point. Later expand_type_access will set the real expressions
    let access = Access {
        creation: CreationAccessExpression {
            precheck: SerializableSlabIndex::shallow(),
        },
        read: SerializableSlabIndex::shallow(),
        update: UpdateAccessExpression {
            precheck: SerializableSlabIndex::shallow(),
            database: SerializableSlabIndex::shallow(),
        },
        delete: SerializableSlabIndex::shallow(),
    };

    let type_validation = match &field.type_hint {
        Some(th) => th.get_type_validation(),
        None => None,
    };

    let default_value = field
        .default_value
        .as_ref()
        .map(|value| match value {
            ResolvedFieldDefault::Value(expr) => match expr.as_ref() {
                AstExpr::StringLiteral(string, _) => Ok(PostgresFieldDefaultValue::Static(
                    Val::String(string.to_string()),
                )),
                AstExpr::BooleanLiteral(boolean, _) => {
                    Ok(PostgresFieldDefaultValue::Static(Val::Bool(*boolean)))
                }
                AstExpr::NumberLiteral(number, _) => {
                    let parsed_number = if field.typ.name() == primitive_type::FloatType::NAME {
                        let float_number = number.parse::<f64>().map_err(|_| {
                            ModelBuildingError::Generic(format!(
                                "Invalid float default value: {}",
                                number
                            ))
                        })?;
                        Ok(ValNumber::F64(float_number))
                    } else if field.typ.name() == primitive_type::IntType::NAME {
                        let int_number = number.parse::<i64>().map_err(|_| {
                            ModelBuildingError::Generic(format!(
                                "Invalid int default value: {}",
                                number
                            ))
                        })?;
                        Ok(ValNumber::I64(int_number))
                    } else {
                        Err(ModelBuildingError::Generic(format!(
                            "Unsupported number type: {}",
                            field.typ.name()
                        )))
                    }?;

                    Ok(PostgresFieldDefaultValue::Static(Val::Number(
                        parsed_number,
                    )))
                }
                AstExpr::FieldSelection(_) => {
                    // Set some value. Will be overridden by expand_dynamic_default_values later
                    Ok(PostgresFieldDefaultValue::Static(Val::Bool(false)))
                }
                _ => Err(ModelBuildingError::Generic(
                    "Unsupported default value expression".to_string(),
                )),
            },
            ResolvedFieldDefault::PostgresFunction(function) => {
                Ok(PostgresFieldDefaultValue::Function(function.to_string()))
            }
            ResolvedFieldDefault::AutoIncrement(name) => {
                Ok(PostgresFieldDefaultValue::AutoIncrement(name.to_owned()))
            }
        })
        .transpose()?;

    Ok(PostgresField {
        name: field.name.to_owned(),
        typ: field.typ.wrap(base_field_type),
        relation,
        access,
        default_value,
        readonly: field.readonly || field.update_sync,
        type_validation,
        doc_comments: field.doc_comments.clone(),
    })
}

fn create_agg_field(
    field: &ResolvedField,
    type_id: &SerializableSlabIndex<EntityType>,
    building: &SystemContextBuilding,
    env: &ResolvedTypeEnv,
    expand_foreign_relations: bool,
) -> Result<Option<AggregateField>, ModelBuildingError> {
    fn is_underlying_type_list(field_type: &FieldType<ResolvedFieldType>) -> bool {
        match field_type {
            FieldType::Plain(_) => false,
            FieldType::Optional(underlying) => is_underlying_type_list(underlying),
            FieldType::List(_) => true,
        }
    }

    if field.typ.innermost().is_primitive || !is_underlying_type_list(&field.typ) {
        Ok(None)
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
        )?);

        Ok(Some(AggregateField {
            name: field_name,
            typ: AggregateFieldType::Composite {
                type_name: agg_type_name,
                type_id: agg_type_id,
            },
            relation,
        }))
    }
}

fn create_vector_distance_field(
    field: &ResolvedField,
    type_id: &SerializableSlabIndex<EntityType>,
    building: &SystemContextBuilding,
    env: &ResolvedTypeEnv,
) -> Option<VectorDistanceField> {
    match &field.type_hint {
        Some(hint) => {
            if let Some(vector_hint) =
                (hint.0.as_ref() as &dyn std::any::Any).downcast_ref::<VectorTypeHint>()
            {
                let self_type = &building.entity_types[*type_id];
                let self_table_id = &self_type.table_id;
                let column_id = building
                    .database
                    .get_column_id(*self_table_id, field.column_name())
                    .unwrap();

                let access = compute_access(&field.access, *type_id, env, building).unwrap();

                Some(VectorDistanceField {
                    name: format!("{}Distance", field.name),
                    column_id,
                    size: vector_hint.size.unwrap_or(DEFAULT_VECTOR_SIZE),
                    distance_function: vector_hint
                        .distance_function
                        .unwrap_or(VectorDistanceFunction::default()),
                    access,
                })
            } else {
                None
            }
        }
        None => None,
    }
}

fn create_relation(
    field: &ResolvedField,
    type_id: SerializableSlabIndex<EntityType>,
    building: &SystemContextBuilding,
    resolved_env: &ResolvedTypeEnv,
    expand_foreign_relations: bool,
) -> Result<PostgresRelation, ModelBuildingError> {
    fn placeholder_relation(is_pk: bool) -> Result<PostgresRelation, ModelBuildingError> {
        // Create an impossible value (will be filled later when expanding relations)
        Ok(PostgresRelation::Scalar {
            column_id: ColumnId {
                table_id: SerializableSlabIndex::from_idx(usize::MAX),
                column_index: usize::MAX,
            },
            is_pk,
        })
    }

    let self_type = &building.entity_types[type_id];
    let self_table_id = &self_type.table_id;

    // we can treat Optional fields as their inner type for the purposes of computing relations
    let field_base_typ = &field.typ.base_type();

    match field_base_typ {
        FieldType::List(underlying) => {
            if self_type.representation == EntityRepresentation::Json {
                Ok(PostgresRelation::Embedded)
            } else {
                // Since the field type is a list, the relation depends on the underlying type.
                // 1. If it is a primitive, we treat it as a scalar ("List" of a primitive type is still a scalar from the database perspective)
                // 2. Otherwise (if it is a composite), it is a one-to-many relation.
                match underlying.deref(resolved_env) {
                    ResolvedType::Primitive(_) => Ok(PostgresRelation::Scalar {
                        column_id: building
                            .database
                            .get_column_id(*self_table_id, field.column_name())
                            .unwrap(),
                        is_pk: false,
                    }),
                    ResolvedType::Enum(_) => Err(ModelBuildingError::Generic(
                        "Enum types are not supported in relations".to_string(),
                    )),
                    ResolvedType::Composite(foreign_field_type) => {
                        if foreign_field_type.representation == EntityRepresentation::Json {
                            Ok(PostgresRelation::Scalar {
                                column_id: building
                                    .database
                                    .get_column_id(*self_table_id, field.column_name())
                                    .unwrap(),
                                is_pk: false,
                            })
                        } else if expand_foreign_relations {
                            compute_many_to_one(
                                field,
                                foreign_field_type,
                                RelationCardinality::Unbounded,
                                building,
                            )
                        } else {
                            placeholder_relation(field.is_pk)
                        }
                    }
                }
            }
        }

        FieldType::Plain(ResolvedFieldType { type_name, .. }) => {
            let foreign_resolved_type = resolved_env.get_by_key(type_name).unwrap();

            match foreign_resolved_type {
                ResolvedType::Primitive(_) | ResolvedType::Enum(_) => {
                    if self_type.representation == EntityRepresentation::Json {
                        Ok(PostgresRelation::Embedded)
                    } else {
                        let column_id = building
                            .database
                            .get_column_id(*self_table_id, field.column_name())
                            .unwrap();
                        Ok(PostgresRelation::Scalar {
                            column_id,
                            is_pk: field.is_pk,
                        })
                    }
                }
                ResolvedType::Composite(foreign_field_type) => {
                    if foreign_field_type.representation == EntityRepresentation::Json {
                        Ok(PostgresRelation::Scalar {
                            column_id: building
                                .database
                                .get_column_id(*self_table_id, field.column_name())
                                .unwrap(),
                            is_pk: field.is_pk,
                        })
                    } else {
                        // A field's type is "Plain" or "Optional" and the field type is composite,
                        // so we need to compute the relation based on the cardinality of the field.
                        match (&field.typ, &field.cardinality) {
                            (FieldType::Optional(_), Some(Cardinality::One)) => {
                                if expand_foreign_relations {
                                    compute_many_to_one(
                                        field,
                                        foreign_field_type,
                                        RelationCardinality::Optional,
                                        building,
                                    )
                                } else {
                                    placeholder_relation(field.is_pk)
                                }
                            }
                            (FieldType::Plain(_), Some(Cardinality::ZeroOrOne)) => {
                                if expand_foreign_relations {
                                    compute_one_to_many_relation(
                                        field,
                                        self_type,
                                        foreign_field_type,
                                        RelationCardinality::Optional,
                                        building,
                                    )
                                } else {
                                    placeholder_relation(field.is_pk)
                                }
                            }
                            (
                                FieldType::Plain(_) | FieldType::Optional(_),
                                Some(Cardinality::Unbounded),
                            ) => {
                                if expand_foreign_relations {
                                    compute_one_to_many_relation(
                                        field,
                                        self_type,
                                        foreign_field_type,
                                        RelationCardinality::Unbounded,
                                        building,
                                    )
                                } else {
                                    placeholder_relation(field.is_pk)
                                }
                            }
                            (FieldType::Plain(_), Some(Cardinality::One)) => {
                                Err(ModelBuildingError::Generic(format!(
                                    "When establishing a one-to-one relation, one side of the relation must be optional. Check the fields of the `{}` and `{}` types.",
                                    self_type.name,
                                    field.typ.name()
                                )))
                            }
                            _ => Err(ModelBuildingError::Generic(format!(
                                "Unexpected relation type for field `{}` of the `{}` type. The matching field is `{}`",
                                field.name,
                                field.typ.name(),
                                foreign_field_type.name
                            ))),
                        }
                    }
                }
            }
        }
        FieldType::Optional(_) => Err(ModelBuildingError::Generic(
            "Nested Optional is not supported".to_string(),
        )),
    }
}

fn compute_many_to_one(
    field: &ResolvedField,
    foreign_field_type: &ResolvedCompositeType,
    cardinality: RelationCardinality,
    building: &SystemContextBuilding,
) -> Result<PostgresRelation, ModelBuildingError> {
    // If the field is of a list type and the underlying type is not a primitive,
    // then it is a OneToMany relation with the self's type being the "One" side
    // and the field's type being the "Many" side.
    let foreign_entity_id = building
        .get_entity_type_id(&foreign_field_type.name)
        .ok_or(ModelBuildingError::Generic(format!(
            "Entity type `{}` not found",
            foreign_field_type.name
        )))?;
    let foreign_type = &building.entity_types[foreign_entity_id];
    let foreign_table_id = foreign_type.table_id;

    let foreign_column_ids: Result<Vec<ColumnId>, ModelBuildingError> = field
        .column_names
        .iter()
        .map(|column_name| {
            building
                .database
                .get_column_id(foreign_table_id, column_name)
                .ok_or(ModelBuildingError::Generic(format!(
                    "Column `{}` not found",
                    column_name
                )))
        })
        .collect();
    let foreign_column_ids = foreign_column_ids?;

    let relation_id = get_otm_relation_for_columns(&foreign_column_ids, &building.database).ok_or(
        ModelBuildingError::Generic(format!(
            "Relation not found for columns `{:?}`",
            field.column_names
        )),
    )?;

    Ok(PostgresRelation::OneToMany(OneToManyRelation {
        foreign_entity_id,
        cardinality,
        relation_id,
    }))
}

fn compute_one_to_many_relation(
    field: &ResolvedField,
    self_type: &EntityType,
    foreign_field_type: &ResolvedCompositeType,
    cardinality: RelationCardinality,
    building: &SystemContextBuilding,
) -> Result<PostgresRelation, ModelBuildingError> {
    let self_table_id = &self_type.table_id;

    let foreign_entity_id = building
        .get_entity_type_id(&foreign_field_type.name)
        .ok_or(ModelBuildingError::Generic(format!(
            "Entity type `{}` not found",
            foreign_field_type.name
        )))?;
    let foreign_type = &building.entity_types[foreign_entity_id];

    let self_column_ids: Result<Vec<ColumnId>, ModelBuildingError> = field
        .column_names
        .iter()
        .map(|name| {
            building.database.get_column_id(*self_table_id, name).ok_or(
                ModelBuildingError::Generic(format!("Column `{}` not found", name)),
            )
        })
        .collect();
    let self_column_ids = self_column_ids?;

    let foreign_pk_field_ids = foreign_type.pk_field_ids(foreign_entity_id);

    let relation_id = get_mto_relation_for_columns(&self_column_ids, &building.database).ok_or(
        ModelBuildingError::Generic(format!(
            "Relation not found for columns `{:?}`",
            field.column_names
        )),
    )?;

    let relation = relation_id.deref(&building.database);
    if relation.column_pairs.len() != foreign_pk_field_ids.len() {
        return Err(ModelBuildingError::Generic(format!(
            "Mismatch between number of columns in relation ({}) and number of foreign PK fields ({}) for field '{}'",
            relation.column_pairs.len(),
            foreign_pk_field_ids.len(),
            field.name
        )));
    }

    Ok(PostgresRelation::ManyToOne {
        relation: ManyToOneRelation {
            cardinality,
            foreign_entity_id,
            foreign_pk_field_ids,
            relation_id,
        },
        is_pk: field.is_pk,
    })
}

fn restrictive_access() -> Access {
    Access {
        creation: CreationAccessExpression {
            precheck: SerializableSlabIndex::shallow(),
        },
        read: SerializableSlabIndex::shallow(),
        update: UpdateAccessExpression {
            precheck: SerializableSlabIndex::shallow(),
            database: SerializableSlabIndex::shallow(),
        },
        delete: SerializableSlabIndex::shallow(),
    }
}
