use core::panic;
use std::ops::Deref;

use id_arena::Id;
use payas_model::{model::{GqlFieldType, column_id::ColumnId, mapped_arena::MappedArena, relation::GqlRelation}, sql::{
        column::{ColumnReferece, PhysicalColumn, PhysicalColumnType},
        PhysicalTable,
    }};

use super::system_builder::SystemContextBuilding;
use super::{
    query_builder,
    typechecking::{PrimitiveType, Type, TypedField},
};

use payas_model::model::{GqlField, GqlType, GqlTypeKind};

pub const PRIMITIVE_TYPE_NAMES: [&str; 2] = ["Int", "String"]; // TODO: Expand the list

pub fn build_shallow(models: &MappedArena<Type>, building: &mut SystemContextBuilding) {
    for type_name in PRIMITIVE_TYPE_NAMES.iter() {
        let typ = GqlType {
            name: type_name.to_string(),
            kind: GqlTypeKind::Primitive,
            is_input: false,
        };
        building.types.add(type_name, typ);
    }

    for (_, model_type) in models.iter() {
        create_shallow_type(model_type, building);
    }
}

pub fn build_expanded(
    env: &MappedArena<Type>,
    building: &mut SystemContextBuilding,
) {
    for (_, model_type) in env.iter() {
        expand_type1(model_type, building, env);
    }
    for (_, model_type) in env.iter() {
        expand_type2(model_type, building, env);
    }
}

fn create_shallow_type(
    model_type: &Type,
    building: &mut SystemContextBuilding,
) {
    if let Type::Composite { name: model_type_name, .. } = &model_type {
        building.types.add(
            &model_type_name,
            GqlType {
                name: model_type_name.to_owned(),
                kind: GqlTypeKind::Primitive,
                is_input: false,
            },
        );

        let mutation_type_names = [
            input_creation_type_name(&model_type_name),
            input_update_type_name(&model_type_name),
            input_reference_type_name(&model_type_name),
        ];

        for mutation_type_name in mutation_type_names.iter() {
            building.mutation_types.add(
                &mutation_type_name,
                GqlType {
                    name: mutation_type_name.to_string(),
                    kind: GqlTypeKind::Primitive,
                    is_input: true,
                },
            );
        }
    } else {
        panic!()
    }
}

// Expand type except for model fields. This allows types to become `Composite` and `table_id` for any type
// can be accessed when building fields
fn expand_type1(model_type: &Type, building: &mut SystemContextBuilding, env: &MappedArena<Type>) {
    if let Type::Composite { name: model_type_name, fields, .. } = &model_type {
        let table_name = model_type
            .get_annotation("table")
            .map(|a| a.params[0].as_string())
            .unwrap_or_else(|| model_type_name.clone());

        let columns = fields
            .iter()
            .flat_map(|field| create_column(field, &table_name, env))
            .collect();

        let table = PhysicalTable {
            name: table_name.clone(),
            columns,
        };
        
        let table_id = building.tables.add(&table_name, table);

        let pk_query = building
            .queries
            .get_id(&query_builder::pk_query_name(&model_type.composite_name()))
            .unwrap();

        let collection_query = building
            .queries
            .get_id(&query_builder::collection_query_name(
                &model_type.composite_name(),
            ))
            .unwrap();

        let kind = GqlTypeKind::Composite {
            fields: vec![],
            table_id,
            pk_query,
            collection_query,
        };
        let existing_type_id = building.types.get_id(&model_type.composite_name());

        building.types.values[existing_type_id.unwrap()].kind = kind;
    } else { panic!() }
}

fn expand_type2(model_type: &Type, building: &mut SystemContextBuilding, env: &MappedArena<Type>) {
    let existing_type_id = building.types.get_id(&model_type.composite_name()).unwrap();
    let existing_type = &building.types[existing_type_id];

    if let GqlTypeKind::Composite {
        table_id,
        pk_query,
        collection_query,
        ..
    } = existing_type.kind
    {
        if let Type::Composite {
            fields,
            name,
            ..
        } = &model_type
        {
            let model_fields: Vec<GqlField> = fields
                .iter()
                .map(|field| create_field(field, table_id, building, env))
                .collect();

            let kind = GqlTypeKind::Composite {
                fields: model_fields.clone(),
                table_id,
                pk_query,
                collection_query,
            };

            building.types.values[existing_type_id].kind = kind;

            {
                let reference_type_fields = model_fields
                    .clone()
                    .into_iter()
                    .flat_map(|field| match &field.relation {
                        GqlRelation::Pk { .. } => Some(field),
                        _ => None,
                    })
                    .collect();

                let existing_type_name = input_reference_type_name(name);
                let existing_type_id = building.mutation_types.get_id(&existing_type_name).unwrap();

                building.mutation_types[existing_type_id].kind = GqlTypeKind::Composite {
                    fields: reference_type_fields,
                    table_id,
                    pk_query,
                    collection_query,
                }
            }

            {
                let input_type_fields = compute_input_fields(&model_fields, building, false);

                let existing_type_name = input_creation_type_name(name);
                let existing_type_id = building.mutation_types.get_id(&existing_type_name).unwrap();

                building.mutation_types[existing_type_id].kind = GqlTypeKind::Composite {
                    fields: input_type_fields,
                    table_id,
                    pk_query,
                    collection_query,
                }
            }

            {
                let input_type_fields = compute_input_fields(&model_fields, building, true);

                let existing_type_name = input_update_type_name(name);
                let existing_type_id = building.mutation_types.get_id(&existing_type_name).unwrap();

                building.mutation_types[existing_type_id].kind = GqlTypeKind::Composite {
                    fields: input_type_fields,
                    table_id,
                    pk_query,
                    collection_query,
                }
            }
        } else {
            panic!()
        }
    }
}

fn compute_input_fields(
    gql_fields: &[GqlField],
    building: &SystemContextBuilding,
    force_optional_field_modifier: bool,
) -> Vec<GqlField> {
    gql_fields
        .iter()
        .flat_map(|field| match &field.relation {
            GqlRelation::Pk { .. } => None,
            GqlRelation::Scalar { .. } => Some(GqlField {
                typ: field.typ.optional(),
                ..field.clone()
            }),
            GqlRelation::ManyToOne { .. } | GqlRelation::OneToMany { .. } => {
                let field_type_name = input_reference_type_name(&field.typ.type_name());
                let field_type_id = building.mutation_types.get_id(&field_type_name).unwrap();
                let field_plain_type = GqlFieldType::Reference {
                    type_name: field_type_name,
                    type_id: field_type_id,
                };
                let field_type = match field.typ {
                    GqlFieldType::Reference { .. } => field_plain_type,
                    GqlFieldType::Optional(_) => {
                        GqlFieldType::Optional(Box::new(field_plain_type))
                    }
                    GqlFieldType::List(_) => GqlFieldType::List(Box::new(field_plain_type)),
                };
                let field_type = if force_optional_field_modifier {
                    field_type.optional()
                } else {
                    field_type
                };
                Some(GqlField {
                    name: field.name.clone(),
                    typ: field_type,
                    relation: field.relation.clone(),
                })
            }
        })
        .collect()
}

fn create_field(
    field: &TypedField,
    table_id: Id<PhysicalTable>,
    building: &SystemContextBuilding,
    env: &MappedArena<Type>,
) -> GqlField {
    fn create_field_type(
        field_type: &Type,
        building: &SystemContextBuilding,
    ) -> GqlFieldType {
        match field_type {
            Type::Reference(r) => GqlFieldType::Reference {
                type_name: r.clone(),
                type_id: building.types.get_id(&r).unwrap(),
            },
            Type::Primitive(p) => {
                let type_name = match p {
                    PrimitiveType::BOOLEAN => "Boolean".to_string(),
                    PrimitiveType::INTEGER => "Int".to_string(),
                    PrimitiveType::STRING => "String".to_string(),
                };
                GqlFieldType::Reference { // TODO(shadaj): fix
                    type_name: type_name.clone(),
                    type_id: building.types.get_id(&type_name).unwrap(),
                }
            },
            Type::Optional(underlying) => GqlFieldType::Optional(Box::new(create_field_type(
                underlying, building,
            ))),
            Type::List(underlying) => {
                GqlFieldType::List(Box::new(create_field_type(underlying, building)))
            }
            o => panic!("Cannot create model type for type {:?}", o),
        }
    }

    GqlField {
        name: field.name.to_owned(),
        typ: create_field_type(&field.typ, building),
        relation: create_relation(&field, table_id, building, env),
    }
}

fn pk_field_of(typ: &Type) -> Option<&TypedField> {
    match &typ {
        Type::Composite { fields, .. } => fields.iter().find(|f| f.get_annotation("pk").is_some()),
        Type::Optional(o) => pk_field_of(o.deref()),
        _ => panic!(),
    }
}

fn create_column(
    field: &TypedField,
    table_name: &str,
    env: &MappedArena<Type>,
) -> Option<PhysicalColumn> {
    match field.get_annotation("pk") {
        Some(_) => Some(PhysicalColumn {
            table_name: table_name.to_string(),
            column_name: field
                .get_annotation("column")
                .map(|a| a.params[0].as_string())
                .unwrap_or_else(|| field.name.clone()),
            typ: PhysicalColumnType::from_string(match field.typ.as_primitive() {
                PrimitiveType::BOOLEAN => "Boolean",
                PrimitiveType::INTEGER => "Int",
                PrimitiveType::STRING => "String",
            }),
            is_pk: true,
            is_autoincrement: match field.get_annotation("autoincrement") {
                Some(_) => {
                    assert!(field.typ == Type::Primitive(PrimitiveType::INTEGER));
                    true
                }
                _ => false,
            },
            references: None,
        }),
        None { .. } => {
            match &field.typ {
                Type::List(_) => None, // OneToMany, so the "many"-side type has the column

                Type::Primitive(_) => {
                    // Scalar type
                    Some(PhysicalColumn {
                        table_name: table_name.to_string(),
                        column_name: field
                            .get_annotation("column")
                            .map(|a| a.params[0].as_string())
                            .unwrap_or_else(|| field.name.clone()),
                        typ: PhysicalColumnType::from_string(match field.typ.as_primitive() {
                            PrimitiveType::BOOLEAN => "Boolean",
                            PrimitiveType::INTEGER => "Int",
                            PrimitiveType::STRING => "String",
                        }),
                        is_pk: false,
                        is_autoincrement: false,
                        references: None,
                    })
                }

                o => {
                    let other_type: Type = o.deref(env);
                    let other_type_pk_field = pk_field_of(&other_type).unwrap();
                    let other_table_name = other_type
                        .get_annotation("table")
                        .map(|a| a.params[0].as_string())
                        .unwrap_or_else(|| other_type.composite_name());

                    Some(PhysicalColumn {
                        table_name: table_name.to_string(),
                        column_name: field
                            .get_annotation("column")
                            .map(|a| a.params[0].as_string())
                            .unwrap_or_else(|| format!("{}_id", field.name)),
                        typ: PhysicalColumnType::from_string(
                            match other_type_pk_field.typ.as_primitive() {
                                PrimitiveType::BOOLEAN => "Boolean",
                                PrimitiveType::INTEGER => "Int",
                                PrimitiveType::STRING => "String",
                            },
                        ),
                        is_pk: false,
                        is_autoincrement: false,
                        references: Some(ColumnReferece {
                            table_name: other_table_name,
                            column_name: field
                                .get_annotation("column")
                                .map(|a| a.params[0].as_string())
                                .unwrap_or_else(|| field.name.clone()),
                        }),
                    })
                }
            }
        }
    }
}

fn create_relation(
    field: &TypedField,
    table_id: Id<PhysicalTable>,
    building: &SystemContextBuilding,
    env: &MappedArena<Type>,
) -> GqlRelation {
    fn compute_column_name(column_name: &Option<String>, field: &TypedField) -> String {
        column_name
            .clone()
            .unwrap_or_else(|| field.name.clone())
    }

    fn compute_column_id(
        table: &PhysicalTable,
        table_id: Id<PhysicalTable>,
        column_name: &Option<String>,
        field: &TypedField,
    ) -> Option<ColumnId> {
        let column_name = compute_column_name(column_name, field);

        table
            .column_index(&column_name)
            .map(|index| ColumnId::new(table_id, index))
    }

    let table = &building.tables[table_id];

    match field.get_annotation("pk") {
        Some(_) => {
            let column_id = compute_column_id(
                table,
                table_id,
                &field
                    .get_annotation("column")
                    .map(|a| a.params[0].as_string()),
                field,
            );
            GqlRelation::Pk {
                column_id: column_id.unwrap(),
            }
        }
        None => {
            match &field.typ {
                // Not primitive
                Type::List(i) => {
                    let other_type_id = building.types.get_id(i.as_ref().inner_composite(env).composite_name().as_str()).unwrap();
                    let other_type = &building.types[other_type_id];
                    let other_table_id = other_type.table_id().unwrap();
                    let other_table = &building.tables[other_table_id];
                    let other_type_column_id = compute_column_id(
                        other_table,
                        other_table_id,
                        &field
                            .get_annotation("column")
                            .map(|a| a.params[0].as_string()),
                        field,
                    )
                    .unwrap();

                    GqlRelation::OneToMany {
                        other_type_column_id,
                        other_type_id,
                    }
                }

                Type::Primitive(_) => {
                    // Primitive
                    let column_id = compute_column_id(
                        table,
                        table_id,
                        &field
                            .get_annotation("column")
                            .map(|a| a.params[0].as_string()),
                        field,
                    );
                    GqlRelation::Scalar {
                        column_id: column_id.unwrap(),
                    }
                }

                o => {
                    // ManyToOne
                    let column_id = compute_column_id(
                        table,
                        table_id,
                        &field
                            .get_annotation("column")
                            .map(|a| a.params[0].as_string()),
                        field,
                    );
                    let other_type_id =
                        building.types.get_id(o.inner_composite(env).composite_name().as_str()).unwrap();
                    GqlRelation::ManyToOne {
                        column_id: column_id.unwrap(),
                        other_type_id,
                        optional: matches!(o, Type::Optional(_)),
                    }
                }
            }
        }
    }
}

pub fn input_creation_type_name(model_type_name: &str) -> String {
    format!("{}CreationInput", model_type_name)
}

pub fn input_update_type_name(model_type_name: &str) -> String {
    format!("{}UpdateInput", model_type_name)
}

pub fn input_reference_type_name(model_type_name: &str) -> String {
    format!("{}ReferenceInput", model_type_name)
}
