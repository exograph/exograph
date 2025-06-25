// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::collections::HashSet;

use crate::naming::ToPlural;
use crate::type_provider::PRIMITIVE_TYPE_PROVIDER_REGISTRY;
use crate::{
    resolved_type::{
        ExplicitTypeHint, ResolvedCompositeType, ResolvedEnumType, ResolvedField,
        ResolvedFieldDefault, ResolvedFieldType, ResolvedType, ResolvedTypeEnv,
    },
    type_provider::VectorTypeHint,
};

use codemap_diagnostic::{Diagnostic, Level, SpanLabel, SpanStyle};
use core_model::{
    primitive_type::{self, PrimitiveType},
    types::FieldType,
};
use core_model_builder::ast::ast_types::{FieldSelection, FieldSelectionElement};
use core_model_builder::{ast::ast_types::AstExpr, error::ModelBuildingError};

use exo_sql::schema::column_spec::{ColumnAutoincrement, ColumnDefault, UuidGenerationMethod};
use exo_sql::{
    ArrayColumnType, BooleanColumnType, ColumnId, EnumColumnType, JsonColumnType, ManyToOne,
    PhysicalColumn, PhysicalColumnType, PhysicalIndex, PhysicalTable, TableId,
    schema::index_spec::IndexKind,
};
use exo_sql::{ColumnReference, Database, PhysicalEnum, RelationColumnPair};

use heck::ToSnakeCase;
use postgres_core_model::types::EntityRepresentation;

struct DatabaseBuilding {
    database: Database,
}

pub fn build(resolved_env: &ResolvedTypeEnv) -> Result<Database, ModelBuildingError> {
    let mut building = DatabaseBuilding {
        database: Database::default(),
    };

    // Ensure that all types have a primary key (skip JSON and unmanaged types)
    for (_, resolved_type) in resolved_env.resolved_types.iter() {
        if let ResolvedType::Composite(c) = &resolved_type {
            if c.representation == EntityRepresentation::Managed && c.pk_fields().is_empty() {
                let diagnostic = Diagnostic {
                    level: Level::Error,
                    message: format!(
                        "Type '{}' has no primary key. Consider annotating one or more fields with @pk",
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
            expand_database_info(c, resolved_env, &mut building)?;
        }
    }

    for (_, resolved_type) in resolved_env.resolved_types.iter() {
        if let ResolvedType::Enum(e) = &resolved_type {
            expand_enum_info(e, &mut building)?;
        }
    }

    for (_, resolved_type) in resolved_env.resolved_types.iter() {
        if let ResolvedType::Composite(c) = &resolved_type {
            expand_type_relations(c, resolved_env, &mut building)?;
        }
    }

    Ok(building.database)
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
fn expand_database_info(
    resolved_type: &ResolvedCompositeType,
    resolved_env: &ResolvedTypeEnv,
    building: &mut DatabaseBuilding,
) -> Result<(), ModelBuildingError> {
    if resolved_type.representation == EntityRepresentation::Json {
        return Ok(());
    }

    let table = PhysicalTable {
        name: resolved_type.table_name.clone(),
        columns: vec![],
        indices: vec![],
        managed: resolved_type.representation == EntityRepresentation::Managed,
    };

    let table_id = building.database.insert_table(table);

    {
        let mut created_columns = HashSet::new();
        let columns: Vec<PhysicalColumn> = resolved_type
            .fields
            .iter()
            .map(|field| {
                create_columns(
                    field,
                    table_id,
                    resolved_type,
                    resolved_env,
                    &mut created_columns,
                )
            })
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .flatten()
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
                        existing_index.columns.extend(field.column_names.clone());
                    }
                    None => indices.push(PhysicalIndex {
                        name: index_name.clone(),
                        columns: HashSet::from_iter(field.column_names.clone()),
                        index_kind: if field.typ.innermost().type_name
                            == primitive_type::VectorType::NAME
                        {
                            let distance_function = field
                                .type_hint
                                .as_ref()
                                .and_then(|hint| {
                                    (hint.0.as_ref() as &dyn std::any::Any)
                                        .downcast_ref::<VectorTypeHint>()
                                        .and_then(|v| v.distance_function)
                                })
                                .unwrap_or_default();

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

    Ok(())
}

fn expand_enum_info(
    resolved_enum: &ResolvedEnumType,
    building: &mut DatabaseBuilding,
) -> Result<(), ModelBuildingError> {
    let table = PhysicalEnum {
        name: resolved_enum.enum_name.clone(),
        variants: resolved_enum.fields.clone(),
    };

    let _ = building.database.insert_enum(table);

    Ok(())
}

fn expand_type_relations(
    resolved_type: &ResolvedCompositeType,
    resolved_env: &ResolvedTypeEnv,
    building: &mut DatabaseBuilding,
) -> Result<(), ModelBuildingError> {
    if resolved_type.representation == EntityRepresentation::Json {
        return Ok(());
    }

    let table_name = &resolved_type.table_name;
    let table_id = building.database.get_table_id(table_name).unwrap();

    resolved_type
        .fields
        .iter()
        .map(|field| -> Result<(), ModelBuildingError> {
            let field_type = resolved_env
                .get_by_key(&field.typ.innermost().type_name)
                .unwrap();
            if let ResolvedType::Composite(ct) = field_type {
                if ct.representation == EntityRepresentation::Json {
                    return Ok(());
                }
            }

            if field.self_column {
                let self_column_ids = building
                    .database
                    .get_column_ids_from_names(table_id, &field.column_names);
                if let Some(relation) = compute_many_to_one_relation(
                    field,
                    self_column_ids.clone(),
                    resolved_env,
                    building,
                )? {
                    // In the earlier phase, we set the type of a many-to-one column to a placeholder value
                    // Now that we have the foreign type, we can set the type of the column to the foreign type's PK
                    for RelationColumnPair {
                        self_column_id,
                        foreign_column_id,
                    } in relation.column_pairs.iter()
                    {
                        let foreign_column_typ =
                            foreign_column_id.get_column(&building.database).typ.clone();

                        building.database.get_column_mut(*self_column_id).typ = foreign_column_typ;

                        let self_column = building.database.get_column_mut(*self_column_id);

                        let column_reference = ColumnReference {
                            foreign_column_id: *foreign_column_id,
                            group_name: field.name.to_string(),
                        };

                        // We may have skipped creating a column if there was a field ahead that referred to the same column
                        // In that case, we need to set the property on the column that we already created
                        self_column.is_pk |= field.is_pk;
                        self_column.update_sync |= field.update_sync;
                        if !matches!(field.typ, FieldType::Optional(_)) {
                            self_column.is_nullable = false;
                        }

                        match self_column.column_references {
                            Some(ref mut column_references) => {
                                column_references.push(column_reference);
                            }
                            None => {
                                self_column.column_references = Some(vec![column_reference]);
                            }
                        }
                    }

                    building.database.relations.push(relation);
                }
            }
            Ok(())
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(())
}

fn default_value(field: &ResolvedField) -> Option<ColumnDefault> {
    field
        .default_value
        .as_ref()
        .and_then(|default_value| match default_value {
            ResolvedFieldDefault::Value(val) => match &**val {
                AstExpr::StringLiteral(string, _) => {
                    let type_name = field.typ.innermost().type_name.as_str();

                    // For Decimal fields, use ColumnDefault::Number to allow proper casting
                    if type_name == primitive_type::DecimalType::NAME {
                        Some(ColumnDefault::Number(string.clone()))
                    } else if type_name == primitive_type::LocalDateType::NAME {
                        Some(ColumnDefault::Date(string.clone()))
                    } else if type_name == primitive_type::LocalTimeType::NAME {
                        Some(ColumnDefault::Time(string.clone()))
                    } else if type_name == primitive_type::LocalDateTimeType::NAME {
                        Some(ColumnDefault::DateTime(string.clone()))
                    } else if type_name == primitive_type::JsonType::NAME {
                        Some(ColumnDefault::Json(string.clone()))
                    } else if type_name == primitive_type::UuidType::NAME {
                        Some(ColumnDefault::UuidLiteral(string.clone()))
                    } else if type_name == primitive_type::BlobType::NAME {
                        Some(ColumnDefault::Function(string.clone()))
                    } else {
                        let value = match field.type_hint {
                            None => ColumnDefault::Text(string.clone()),
                            Some(_) => ColumnDefault::VarChar(string.clone()),
                        };
                        Some(value)
                    }
                }
                AstExpr::BooleanLiteral(boolean, _) => Some(ColumnDefault::Boolean(*boolean)),
                AstExpr::NumberLiteral(val, _) => Some(ColumnDefault::Number(val.clone())),
                AstExpr::FieldSelection(selection) => match selection {
                    FieldSelection::Single(element, _) => match element {
                        FieldSelectionElement::Identifier(value, _, _) => {
                            Some(ColumnDefault::Enum(value.clone()))
                        }
                        FieldSelectionElement::HofCall { .. } => None,
                        FieldSelectionElement::NormalCall { .. } => None,
                    },
                    FieldSelection::Select(_, _, _, _) => None,
                },
                _ => panic!("Invalid concrete value"),
            },
            ResolvedFieldDefault::PostgresFunction(string) => {
                if string == "now()" {
                    if field.typ.innermost().type_name.as_str()
                        == primitive_type::LocalDateType::NAME
                    {
                        Some(ColumnDefault::CurrentDate)
                    } else {
                        Some(ColumnDefault::CurrentTimestamp)
                    }
                } else if string == "gen_random_uuid()" {
                    Some(ColumnDefault::Uuid(UuidGenerationMethod::GenRandomUuid))
                } else if string == "uuid_generate_v4()" {
                    Some(ColumnDefault::Uuid(UuidGenerationMethod::UuidGenerateV4))
                } else {
                    Some(ColumnDefault::Function(string.to_string()))
                }
            }
            ResolvedFieldDefault::AutoIncrement(value) => {
                Some(ColumnDefault::Autoincrement(match value {
                    Some(sequence_name) => ColumnAutoincrement::Sequence {
                        name: sequence_name.clone(),
                    },
                    None => ColumnAutoincrement::Serial,
                }))
            }
        })
}

fn create_columns(
    field: &ResolvedField,
    table_id: TableId,
    resolved_type: &ResolvedCompositeType,
    env: &ResolvedTypeEnv,
    created_columns: &mut HashSet<String>,
) -> Result<Vec<PhysicalColumn>, ModelBuildingError> {
    // If the field doesn't have a column in the same table (for example, the `concerts` field in the `Venue` type), skip it
    if !field.self_column {
        return Ok(vec![]);
    }

    let unique_constraint_name: Vec<String> = field
        .unique_constraints
        .iter()
        .map(|constraint| {
            format!("unique_constraint_{}_{}", resolved_type.name, constraint).to_snake_case()
        })
        .collect();

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
                ResolvedType::Primitive(pt) => Ok(field
                    .column_names
                    .iter()
                    .flat_map(|column_name| {
                        if created_columns.insert(column_name.to_string()) {
                            Some(PhysicalColumn {
                                table_id,
                                name: column_name.to_string(),
                                typ: determine_column_type(pt, field),
                                is_pk: field.is_pk,
                                is_nullable: optional,
                                unique_constraints: unique_constraint_name.clone(),
                                default_value: default_value.clone(),
                                update_sync,
                                column_references: None,
                            })
                        } else {
                            None
                        }
                    })
                    .collect()),
                ResolvedType::Composite(composite) => {
                    // Many-to-one:
                    // Column from the current table (but of the type of the pk column of the other table)
                    // and it refers to the pk column in the other table.

                    Ok(field
                        .column_names
                        .iter()
                        .flat_map(|column_name| {
                            if created_columns.insert(column_name.to_string()) {
                                Some(PhysicalColumn {
                                    table_id,
                                    name: column_name.to_string(),
                                    typ: if composite.representation == EntityRepresentation::Json {
                                        Box::new(JsonColumnType)
                                    } else {
                                        // A placeholder value. Will be resolved in the next phase (see expand_type_relations)
                                        Box::new(BooleanColumnType)
                                    },
                                    is_pk: field.is_pk,
                                    is_nullable: optional,
                                    unique_constraints: unique_constraint_name.clone(),
                                    default_value: default_value.clone(),
                                    update_sync,
                                    column_references: None,
                                })
                            } else {
                                None
                            }
                        })
                        .collect())
                }
                ResolvedType::Enum(enum_type) => Ok(field
                    .column_names
                    .iter()
                    .map(|name| PhysicalColumn {
                        table_id,
                        name: name.to_string(),
                        typ: Box::new(EnumColumnType {
                            enum_name: enum_type.enum_name.clone(),
                        }),
                        is_pk: field.is_pk,
                        is_nullable: optional,
                        unique_constraints: unique_constraint_name.clone(),
                        default_value: default_value.clone(),
                        update_sync,
                        column_references: None,
                    })
                    .collect()),
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

                Ok(field
                    .column_names
                    .iter()
                    .map(|name| PhysicalColumn {
                        table_id,
                        name: name.to_string(),
                        typ: determine_column_type(&pt, field),
                        is_pk: false,
                        is_nullable: optional,
                        unique_constraints: unique_constraint_name.clone(),
                        default_value: default_value.clone(),
                        update_sync,
                        column_references: None,
                    })
                    .collect())
            } else {
                // this is a OneToMany relation, so the other side has the associated column
                Ok(vec![])
            }
        }
        FieldType::Optional(_) => Err(ModelBuildingError::Generic(
            "Optional in an Optional? is not supported".to_string(),
        )),
    }
}

fn compute_many_to_one_relation(
    field: &ResolvedField,
    self_column_ids: Vec<ColumnId>,
    env: &ResolvedTypeEnv,
    building: &mut DatabaseBuilding,
) -> Result<Option<ManyToOne>, ModelBuildingError> {
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
                    let foreign_pk_column_ids =
                        building.database.get_pk_column_ids(foreign_table_id);

                    let field_alias = field.name.to_snake_case().to_plural();

                    if self_column_ids.len() != foreign_pk_column_ids.len() {
                        return Err(ModelBuildingError::Generic(format!(
                            "Mismatch between number of self columns ({}) and foreign primary key columns ({}) for field '{}'",
                            self_column_ids.len(),
                            foreign_pk_column_ids.len(),
                            field.name
                        )));
                    }

                    Ok(Some(ManyToOne::new(
                        self_column_ids
                            .into_iter()
                            .zip(foreign_pk_column_ids)
                            .map(|(self_column_id, foreign_column_id)| RelationColumnPair {
                                self_column_id,
                                foreign_column_id,
                            })
                            .collect(),
                        Some(field_alias),
                    )))
                }
                _ => Ok(None),
            }
        }
        _ => Ok(None),
    }
}

fn determine_column_type<'a>(
    pt: &'a PrimitiveType,
    field: &'a ResolvedField,
) -> Box<dyn PhysicalColumnType> {
    // Check for explicit type hints first
    if let Some(hint) = &field.type_hint {
        let hint_ref = hint.0.as_ref() as &dyn std::any::Any;
        if let Some(explicit) = hint_ref.downcast_ref::<ExplicitTypeHint>() {
            return exo_sql::schema::column_spec::physical_column_type_from_string(
                &explicit.dbtype,
                &vec![],
            )
            .unwrap();
        }
    }

    match pt {
        PrimitiveType::Array(underlying_pt) => {
            let inner_type = determine_column_type(underlying_pt, field);
            Box::new(ArrayColumnType { typ: inner_type })
        }
        PrimitiveType::Plain(base_pt_type) => {
            if let Some(provider) = PRIMITIVE_TYPE_PROVIDER_REGISTRY.get(base_pt_type.name()) {
                provider.determine_column_type(field)
            } else {
                panic!("Unknown primitive type: {:?}", base_pt_type);
            }
        }
    }
}
