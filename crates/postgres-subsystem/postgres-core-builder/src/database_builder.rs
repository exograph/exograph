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
use crate::resolved_type::{
    ResolvedCompositeType, ResolvedField, ResolvedFieldDefault, ResolvedFieldType,
    ResolvedFieldTypeHelper, ResolvedType, ResolvedTypeEnv, ResolvedTypeHint,
};

use codemap_diagnostic::{Diagnostic, Level, SpanLabel, SpanStyle};
use core_plugin_interface::core_model_builder::ast::ast_types::AstExpr;
use core_plugin_interface::{
    core_model::{primitive_type::PrimitiveType, types::FieldType},
    core_model_builder::error::ModelBuildingError,
};

use exo_sql::Database;
use exo_sql::{
    schema::index_spec::IndexKind, ColumnId, FloatBits, IntBits, ManyToOne, PhysicalColumn,
    PhysicalColumnType, PhysicalIndex, PhysicalTable, TableId, VectorDistanceFunction,
    DEFAULT_VECTOR_SIZE,
};

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
            if c.representation == EntityRepresentation::Managed && c.pk_field().is_none() {
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
            expand_database_info(c, resolved_env, &mut building)?;
        }
    }

    for (_, resolved_type) in resolved_env.resolved_types.iter() {
        if let ResolvedType::Composite(c) = &resolved_type {
            expand_type_relations(c, resolved_env, &mut building);
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
        let columns = resolved_type
            .fields
            .iter()
            .map(|field| create_columns(field, table_id, resolved_type, resolved_env))
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

    Ok(())
}

fn expand_type_relations(
    resolved_type: &ResolvedCompositeType,
    resolved_env: &ResolvedTypeEnv,
    building: &mut DatabaseBuilding,
) {
    if resolved_type.representation == EntityRepresentation::Json {
        return;
    }

    let table_name = &resolved_type.table_name;
    let table_id = building.database.get_table_id(table_name).unwrap();

    resolved_type.fields.iter().for_each(|field| {
        let field_type = resolved_env
            .get_by_key(&field.typ.innermost().type_name)
            .unwrap();
        if let ResolvedType::Composite(ct) = field_type {
            if ct.representation == EntityRepresentation::Json {
                return;
            }
        }

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

fn create_columns(
    field: &ResolvedField,
    table_id: TableId,
    resolved_type: &ResolvedCompositeType,
    env: &ResolvedTypeEnv,
) -> Result<Vec<PhysicalColumn>, ModelBuildingError> {
    // If the field doesn't have a column in the same table (for example, the `concerts` field in the `Venue` type), skip it
    if !field.self_column {
        return Ok(vec![]);
    }

    let unique_constraint_name = field
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
                ResolvedType::Primitive(pt) => Ok(vec![PhysicalColumn {
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
                }]),
                ResolvedType::Composite(composite) => {
                    // Many-to-one:
                    // Column from the current table (but of the type of the pk column of the other table)
                    // and it refers to the pk column in the other table.
                    // Ok(composite
                    //     .fields
                    //     .iter()
                    //     .filter(|field| field.is_pk)
                    //     .map(|field| {
                    //         println!("creating column: {:?} in {}", field, &composite.name);
                    //         PhysicalColumn {
                    //             table_id,
                    //             // name: format!("{}_{}", composite.name, field.column_name),
                    //             name: field.column_name.to_string(),
                    //             typ: if composite.representation == EntityRepresentation::Json {
                    //                 PhysicalColumnType::Json
                    //             } else {
                    //                 // A placeholder value. Will be resolved in the next phase (see expand_type_relations)
                    //                 PhysicalColumnType::Boolean
                    //             },
                    //             is_pk: false,
                    //             is_auto_increment: false,
                    //             is_nullable: optional,
                    //             unique_constraints: unique_constraint_name.clone(),
                    //             default_value: default_value.clone(),
                    //             update_sync,
                    //         }
                    //     })
                    //     .collect())

                    println!(
                        "creating column: {:?} in {}",
                        field.column_name, &composite.name
                    );

                    Ok(vec![PhysicalColumn {
                        table_id,
                        name: field.column_name.to_string(),
                        typ: if composite.representation == EntityRepresentation::Json {
                            PhysicalColumnType::Json
                        } else {
                            // A placeholder value. Will be resolved in the next phase (see expand_type_relations)
                            PhysicalColumnType::Boolean
                        },
                        is_pk: false,
                        is_auto_increment: false,
                        is_nullable: optional,
                        unique_constraints: unique_constraint_name,
                        default_value,
                        update_sync,
                    }])
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

                Ok(vec![PhysicalColumn {
                    table_id,
                    name: field.column_name.to_string(),
                    typ: determine_column_type(&pt, field),
                    is_pk: false,
                    is_auto_increment: false,
                    is_nullable: optional,
                    unique_constraints: unique_constraint_name,
                    default_value,
                    update_sync,
                }])
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
    self_column_id: ColumnId,
    env: &ResolvedTypeEnv,
    building: &mut DatabaseBuilding,
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
