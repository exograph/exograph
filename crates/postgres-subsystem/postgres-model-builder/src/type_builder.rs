// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::collections::{HashMap, HashSet};

use crate::{
    aggregate_type_builder::aggregate_type_name, naming::ToPlural,
    resolved_builder::ResolvedFieldTypeHelper, shallow::Shallow,
};

use super::{access_builder::ResolvedAccess, access_utils, resolved_builder::ResolvedFieldType};

use codemap_diagnostic::{Diagnostic, Level, SpanLabel, SpanStyle};
use core_plugin_interface::{
    core_model::{
        access::AccessPredicateExpression,
        context_type::ContextType,
        function_defn::FunctionDefinition,
        mapped_arena::{MappedArena, SerializableSlabIndex},
        primitive_type::PrimitiveType,
        types::{FieldType, Named},
    },
    core_model_builder::{ast::ast_types::AstExpr, error::ModelBuildingError, typechecker::Typed},
};

use exo_sql::{
    schema::index_spec::IndexKind, ColumnId, FloatBits, IntBits, ManyToOne, PhysicalColumn,
    PhysicalColumnType, PhysicalIndex, PhysicalTable, TableId, VectorDistanceFunction,
    DEFAULT_VECTOR_SIZE,
};

use heck::ToSnakeCase;
use postgres_model::{
    access::{
        Access, DatabaseAccessPrimitiveExpression, InputAccessPrimitiveExpression,
        UpdateAccessExpression,
    },
    aggregate::{AggregateField, AggregateFieldType},
    relation::{ManyToOneRelation, OneToManyRelation, PostgresRelation, RelationCardinality},
    types::{
        get_field_id, EntityType, PostgresField, PostgresFieldType, PostgresPrimitiveType,
        TypeIndex, TypeValidationProvider,
    },
    vector_distance::{VectorDistanceField, VectorDistanceType},
};

use super::{
    naming::ToPostgresQueryName,
    resolved_builder::{
        ResolvedCompositeType, ResolvedField, ResolvedFieldDefault, ResolvedType, ResolvedTypeHint,
    },
    system_builder::SystemContextBuilding,
};

#[derive(Debug, Clone)]
pub struct ResolvedTypeEnv<'a> {
    pub contexts: &'a MappedArena<ContextType>,
    pub resolved_types: MappedArena<ResolvedType>,
    pub function_definitions: &'a MappedArena<FunctionDefinition>,
}

impl<'a> ResolvedTypeEnv<'a> {
    pub fn get_by_key(&self, key: &str) -> Option<&ResolvedType> {
        self.resolved_types.get_by_key(key)
    }
}

pub(super) fn build_shallow(resolved_env: &ResolvedTypeEnv, building: &mut SystemContextBuilding) {
    for (_, resolved_type) in resolved_env.resolved_types.iter() {
        create_shallow_type(resolved_type, resolved_env, building);
    }
}

pub(super) fn build_expanded(
    resolved_env: &ResolvedTypeEnv,
    building: &mut SystemContextBuilding,
) -> Result<(), ModelBuildingError> {
    // Ensure that all types have a primary key
    for (_, resolved_type) in resolved_env.resolved_types.iter() {
        if let ResolvedType::Composite(c) = &resolved_type {
            if c.pk_field().is_none() {
                let diagnostic = Diagnostic {
                    level: Level::Error,
                    message: format!(
                        "Type '{}' has no primary key. Consider annotating a field with @pk",
                        c.name
                    ),
                    code: Some("C000".to_string()),
                    spans: vec![SpanLabel {
                        span: c.span,
                        style: SpanStyle::Primary,
                        label: None,
                    }],
                };

                Err(ModelBuildingError::Diagnosis(vec![diagnostic]))?;
            }
        }
    }

    for (_, resolved_type) in resolved_env.resolved_types.iter() {
        if let ResolvedType::Composite(c) = &resolved_type {
            expand_type_no_fields(c, resolved_env, building);
        }
    }

    for (_, resolved_type) in resolved_env.resolved_types.iter() {
        if let ResolvedType::Composite(c) = &resolved_type {
            expand_type_relations(c, resolved_env, building);
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
        ResolvedType::Composite(_) => {
            let typ = EntityType {
                name: resolved_type.name(),
                plural_name: resolved_type.plural_name(),
                fields: vec![],
                agg_fields: vec![],
                vector_distance_fields: vec![],
                table_id: SerializableSlabIndex::shallow(),
                pk_query: SerializableSlabIndex::shallow(),
                collection_query: SerializableSlabIndex::shallow(),
                aggregate_query: SerializableSlabIndex::shallow(),
                access: restrictive_access(),
            };

            building.entity_types.add(&resolved_type.name(), typ);
        }
    }
}

/// Expand a composite type except for creating its fields.
///
/// Specifically:
/// 1. Create and set the table along with its columns. However, columns will not have its references set
/// 2. Create and set *_query members (if applicable)
/// 3. Leave fields as an empty vector
///
/// This allows the type to become `Composite` and `table_id` for any type can be accessed when building fields in the next step of expansion.
/// We can't expand fields yet since creating a field requires access to columns (self as well as those in a referred field in case a relation)
/// and we may not have expanded a referred type yet.
fn expand_type_no_fields(
    resolved_type: &ResolvedCompositeType,
    resolved_env: &ResolvedTypeEnv,
    building: &mut SystemContextBuilding,
) {
    let table = PhysicalTable {
        name: resolved_type.table_name.clone(),
        columns: vec![],
        indices: vec![],
    };

    let table_id = building.database.insert_table(table);

    {
        let columns = resolved_type
            .fields
            .iter()
            .flat_map(|field| create_column(field, table_id, resolved_type, resolved_env))
            .collect();
        building.database.get_table_mut(table_id).columns = columns;
    }

    {
        let mut indices: Vec<PhysicalIndex> = vec![];
        resolved_type.fields.iter().for_each(|field| {
            field.indices.iter().for_each(|index_name| {
                let existing_index = indices.iter_mut().find(|i| &i.name == index_name);

                match existing_index {
                    Some(existing_index) => {
                        existing_index.columns.insert(field.column_name.clone());
                    }
                    None => indices.push(PhysicalIndex {
                        name: index_name.clone(),
                        columns: HashSet::from_iter([field.column_name.clone()]),
                        index_kind: if field.typ.innermost().type_name == "Vector" {
                            let distance_function = match field.type_hint {
                                Some(ResolvedTypeHint::Vector {
                                    distance_function, ..
                                }) => distance_function,
                                _ => None,
                            }
                            .unwrap_or(VectorDistanceFunction::default());

                            IndexKind::HNWS {
                                distance_function,
                                params: None,
                            }
                        } else {
                            IndexKind::default()
                        },
                    }),
                }
            })
        });
        building.database.get_table_mut(table_id).indices = indices;
    }

    let pk_query = building
        .pk_queries
        .get_id(&resolved_type.pk_query())
        .unwrap();

    let collection_query = building
        .collection_queries
        .get_id(&resolved_type.collection_query())
        .unwrap();

    let aggregate_query = building
        .aggregate_queries
        .get_id(&resolved_type.aggregate_query())
        .unwrap();

    let existing_type_id = building.get_entity_type_id(&resolved_type.name).unwrap();
    let existing_type = &mut building.entity_types[existing_type_id];
    existing_type.table_id = table_id;
    existing_type.pk_query = pk_query;
    existing_type.collection_query = collection_query;
    existing_type.aggregate_query = aggregate_query;
}

fn expand_type_relations(
    resolved_type: &ResolvedCompositeType,
    resolved_env: &ResolvedTypeEnv,
    building: &mut SystemContextBuilding,
) {
    let table_name = &resolved_type.table_name;
    let table_id = building.database.get_table_id(table_name).unwrap();

    resolved_type.fields.iter().for_each(|field| {
        if field.self_column {
            let self_column_id = building
                .database
                .get_column_id(table_id, &field.column_name)
                .unwrap();
            if let Some(relation) =
                compute_many_to_one_relation(field, self_column_id, resolved_env, building)
            {
                // In the earlier phase, we set the type of a many-to-one column to a placeholder value
                // Now that we have the foreign type, we can set the type of the column to the foreign type's PK
                let foreign_column_typ = &relation
                    .foreign_pk_column_id
                    .get_column(&building.database)
                    .typ;
                building.database.get_column_mut(self_column_id).typ = foreign_column_typ.clone();
                building.database.relations.push(relation);
            }
        }
    });
}

/// Now that all types have table with them (set in the earlier expand_type_no_fields phase), we can
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
            .borrow()
            .restricted_access_index(),
        Some(expr) => building
            .database_access_expressions
            .borrow_mut()
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
            .borrow()
            .restricted_access_index(),
        Some(expr) => building.input_access_expressions.borrow_mut().insert(expr),
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

fn default_value(field: &ResolvedField) -> Option<String> {
    field
        .default_value
        .as_ref()
        .and_then(|default_value| match default_value {
            ResolvedFieldDefault::Value(val) => match &**val {
                AstExpr::StringLiteral(string, _) => {
                    Some(format!("'{}'::text", string.replace('\'', "''")))
                }
                AstExpr::BooleanLiteral(boolean, _) => Some(format!("{boolean}")),
                AstExpr::NumberLiteral(val, _) => Some(format!("{val}")),
                AstExpr::FieldSelection(_) => None,
                _ => panic!("Invalid concrete value"),
            },
            ResolvedFieldDefault::PostgresFunction(string) => Some(string.to_string()),
            ResolvedFieldDefault::AutoIncrement => None,
        })
}

fn create_column(
    field: &ResolvedField,
    table_id: TableId,
    resolved_type: &ResolvedCompositeType,
    env: &ResolvedTypeEnv,
) -> Option<PhysicalColumn> {
    // Check that the field holds to a self column
    let unique_constraint_name = if !field.self_column {
        return None;
    } else {
        field
            .unique_constraints
            .iter()
            .map(|constraint| {
                format!("unique_constraint_{}_{}", resolved_type.name, constraint).to_snake_case()
            })
            .collect()
    };
    // split a Optional type into its inner type and the optional marker
    let (typ, optional) = match &field.typ {
        FieldType::Optional(inner_typ) => (inner_typ.as_ref(), true),
        _ => (&field.typ, false),
    };

    let default_value = default_value(field);
    let update_sync = field.update_sync;

    match typ {
        FieldType::Plain(ResolvedFieldType { type_name, .. }) => {
            // Either a scalar (primitive) or a many-to-one relationship with another table
            let field_type = env.get_by_key(type_name).unwrap();

            match field_type {
                ResolvedType::Primitive(pt) => Some(PhysicalColumn {
                    table_id,
                    name: field.column_name.to_string(),
                    typ: determine_column_type(pt, field),
                    is_pk: field.is_pk,
                    is_auto_increment: if field.get_is_auto_increment() {
                        assert!(matches!(
                            typ.deref(env),
                            &ResolvedType::Primitive(PrimitiveType::Int)
                        ));
                        true
                    } else {
                        false
                    },
                    is_nullable: optional,
                    unique_constraints: unique_constraint_name,
                    default_value,
                    update_sync,
                }),
                ResolvedType::Composite(_) => {
                    // Many-to-one:
                    // Column from the current table (but of the type of the pk column of the other table)
                    // and it refers to the pk column in the other table.
                    Some(PhysicalColumn {
                        table_id,
                        name: field.column_name.to_string(),
                        typ: PhysicalColumnType::Boolean, // A placeholder value. Will be resolved in the next phase (see expand_type_relations)
                        is_pk: false,
                        is_auto_increment: false,
                        is_nullable: optional,
                        unique_constraints: unique_constraint_name,
                        default_value,
                        update_sync,
                    })
                }
            }
        }
        FieldType::List(typ) => {
            // unwrap list to base type
            let mut underlying_typ = typ;
            let mut depth = 1;

            while let FieldType::List(t) = &**underlying_typ {
                underlying_typ = t;
                depth += 1;
            }

            let underlying_pt = if let FieldType::Plain(ResolvedFieldType { type_name, .. }) =
                &**underlying_typ
            {
                if let Some(ResolvedType::Primitive(pt)) = env.resolved_types.get_by_key(type_name)
                {
                    Some(pt)
                } else {
                    None
                }
            } else {
                todo!()
            };

            // is our underlying list type a primitive or a column?
            if let Some(underlying_pt) = underlying_pt {
                // underlying type is a primitive, so treat it as an Array

                // rewrap underlying PrimitiveType
                let mut pt = underlying_pt.clone();
                for _ in 0..depth {
                    pt = PrimitiveType::Array(Box::new(pt))
                }

                Some(PhysicalColumn {
                    table_id,
                    name: field.column_name.to_string(),
                    typ: determine_column_type(&pt, field),
                    is_pk: false,
                    is_auto_increment: false,
                    is_nullable: optional,
                    unique_constraints: unique_constraint_name,
                    default_value,
                    update_sync,
                })
            } else {
                // this is a OneToMany relation, so the other side has the associated column
                None
            }
        }
        FieldType::Optional(_) => panic!("Optional in an Optional?"),
    }
}

fn compute_many_to_one_relation(
    field: &ResolvedField,
    self_column_id: ColumnId,
    env: &ResolvedTypeEnv,
    building: &SystemContextBuilding,
) -> Option<ManyToOne> {
    let typ = match &field.typ {
        FieldType::Optional(inner_typ) => inner_typ.as_ref(),
        _ => &field.typ,
    };
    match typ {
        FieldType::Plain(ResolvedFieldType { type_name, .. }) => {
            let field_type = env.get_by_key(type_name).unwrap();
            match field_type {
                ResolvedType::Composite(ct) => {
                    // Column from the current table (but of the type of the pk column of the other table)
                    // and it refers to the pk column in the other table.

                    let foreign_table_id = building.database.get_table_id(&ct.table_name).unwrap();
                    let foreign_pk_column_id = building
                        .database
                        .get_pk_column_id(foreign_table_id)
                        .unwrap();

                    let field_alias = field.name.to_snake_case().to_plural();

                    Some(ManyToOne {
                        self_column_id,
                        foreign_pk_column_id,
                        foreign_table_alias: Some(field_alias),
                    })
                }
                _ => None,
            }
        }
        _ => None,
    }
}

fn determine_column_type<'a>(
    pt: &'a PrimitiveType,
    field: &'a ResolvedField,
) -> PhysicalColumnType {
    if let PrimitiveType::Array(underlying_pt) = pt {
        return PhysicalColumnType::Array {
            typ: Box::new(determine_column_type(underlying_pt, field)),
        };
    }

    if let Some(hint) = &field.type_hint {
        match hint {
            ResolvedTypeHint::Explicit { dbtype } => {
                PhysicalColumnType::from_string(dbtype).unwrap()
            }

            ResolvedTypeHint::Int { bits, range } => {
                assert!(matches!(pt, PrimitiveType::Int));

                // determine the proper sized type to use
                if let Some(bits) = bits {
                    PhysicalColumnType::Int {
                        bits: match bits {
                            16 => IntBits::_16,
                            32 => IntBits::_32,
                            64 => IntBits::_64,
                            _ => panic!("Invalid bits"),
                        },
                    }
                } else if let Some(range) = range {
                    let is_superset = |bound_min: i64, bound_max: i64| {
                        let range_min = range.0;
                        let range_max = range.1;
                        assert!(range_min <= range_max);
                        assert!(bound_min <= bound_max);

                        // is this bound a superset of the provided range?
                        (bound_min <= range_min && bound_min <= range_max)
                            && (bound_max >= range_max && bound_max >= range_min)
                    };

                    // determine which SQL type is appropriate for this range
                    {
                        if is_superset(i16::MIN.into(), i16::MAX.into()) {
                            PhysicalColumnType::Int { bits: IntBits::_16 }
                        } else if is_superset(i32::MIN.into(), i32::MAX.into()) {
                            PhysicalColumnType::Int { bits: IntBits::_32 }
                        } else if is_superset(i64::MIN, i64::MAX) {
                            PhysicalColumnType::Int { bits: IntBits::_64 }
                        } else {
                            // TODO: numeric type
                            panic!("Requested range is too big")
                        }
                    }
                } else {
                    // no hints provided, go with default
                    PhysicalColumnType::Int { bits: IntBits::_32 }
                }
            }

            ResolvedTypeHint::Float { bits, .. } => {
                assert!(matches!(pt, PrimitiveType::Float));

                if let Some(bits) = *bits {
                    if (1..=24).contains(&bits) {
                        PhysicalColumnType::Float {
                            bits: FloatBits::_24,
                        }
                    } else if bits > 24 && bits <= 53 {
                        PhysicalColumnType::Float {
                            bits: FloatBits::_53,
                        }
                    } else {
                        panic!("Invalid bits")
                    }
                } else {
                    PhysicalColumnType::Float {
                        bits: FloatBits::_53,
                    }
                }
            }

            ResolvedTypeHint::Decimal { precision, scale } => {
                assert!(matches!(pt, PrimitiveType::Decimal));

                // cannot have scale and no precision specified
                if precision.is_none() {
                    assert!(scale.is_none())
                }

                PhysicalColumnType::Numeric {
                    precision: *precision,
                    scale: *scale,
                }
            }

            ResolvedTypeHint::String { max_length } => {
                assert!(matches!(pt, PrimitiveType::String));

                // length hint provided, use it
                PhysicalColumnType::String {
                    max_length: Some(*max_length),
                }
            }

            ResolvedTypeHint::DateTime { precision } => match pt {
                PrimitiveType::LocalTime => PhysicalColumnType::Time {
                    precision: Some(*precision),
                },
                PrimitiveType::LocalDateTime => PhysicalColumnType::Timestamp {
                    precision: Some(*precision),
                    timezone: false,
                },
                PrimitiveType::Instant => PhysicalColumnType::Timestamp {
                    precision: Some(*precision),
                    timezone: true,
                },
                _ => panic!(),
            },

            ResolvedTypeHint::Vector { size, .. } => {
                assert!(matches!(pt, PrimitiveType::Vector));

                PhysicalColumnType::Vector {
                    size: (*size).unwrap_or(DEFAULT_VECTOR_SIZE),
                }
            }
        }
    } else {
        match pt {
            // choose a default SQL type
            PrimitiveType::Int => PhysicalColumnType::Int { bits: IntBits::_32 },
            PrimitiveType::Float => PhysicalColumnType::Float {
                bits: FloatBits::_24,
            },
            PrimitiveType::Decimal => PhysicalColumnType::Numeric {
                precision: None,
                scale: None,
            },
            PrimitiveType::String => PhysicalColumnType::String { max_length: None },
            PrimitiveType::Boolean => PhysicalColumnType::Boolean,
            PrimitiveType::LocalTime => PhysicalColumnType::Time { precision: None },
            PrimitiveType::LocalDateTime => PhysicalColumnType::Timestamp {
                precision: None,
                timezone: false,
            },
            PrimitiveType::LocalDate => PhysicalColumnType::Date,
            PrimitiveType::Instant => PhysicalColumnType::Timestamp {
                precision: None,
                timezone: true,
            },
            PrimitiveType::Json => PhysicalColumnType::Json,
            PrimitiveType::Blob => PhysicalColumnType::Blob,
            PrimitiveType::Uuid => PhysicalColumnType::Uuid,
            PrimitiveType::Vector => PhysicalColumnType::Vector {
                size: DEFAULT_VECTOR_SIZE,
            },
            PrimitiveType::Array(_)
            | PrimitiveType::Exograph
            | PrimitiveType::ExographPriv
            | PrimitiveType::Interception(_) => {
                panic!()
            }
        }
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
                        if expand_foreign_relations {
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

            FieldType::Plain(ResolvedFieldType { type_name, .. }) => {
                let foreign_resolved_type = resolved_env.get_by_key(type_name).unwrap();

                match foreign_resolved_type {
                    ResolvedType::Primitive(_) => {
                        let column_id = building
                            .database
                            .get_column_id(*self_table_id, &field.column_name)
                            .unwrap();
                        PostgresRelation::Scalar { column_id }
                    }
                    ResolvedType::Composite(foreign_field_type) => {
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
