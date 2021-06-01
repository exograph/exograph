use core::panic;
use std::{collections::HashMap, ops::Deref};

use id_arena::Id;
use payas_model::{
    model::{column_id::ColumnId, relation::ModelRelation, ModelFieldType},
    sql::{
        column::{ColumnReferece, PhysicalColumn, PhysicalColumnType},
        PhysicalTable,
    },
};

use super::system_builder::SystemContextBuilding;
use super::{
    query_builder,
    typechecking::{PrimitiveType, Type, TypedField},
};

use payas_model::model::{ModelField, ModelType, ModelTypeKind};

pub const PRIMITIVE_TYPE_NAMES: [&str; 2] = ["Int", "String"]; // TODO: Expand the list

pub fn build_shallow(ast_models: &HashMap<String, &Type>, building: &mut SystemContextBuilding) {
    for type_name in PRIMITIVE_TYPE_NAMES.iter() {
        let typ = ModelType {
            name: type_name.to_string(),
            kind: ModelTypeKind::Primitive,
            is_input: false,
        };
        building.types.add(type_name, typ);
    }

    for ast_type in ast_models.values() {
        create_shallow_type(ast_type, ast_models, building);
    }
}

pub fn build_expanded(
    ast_types_map: &HashMap<String, &Type>,
    building: &mut SystemContextBuilding,
) {
    for ast_type in ast_types_map.values() {
        expand_type1(ast_type, building);
    }
    for ast_type in ast_types_map.values() {
        expand_type2(ast_type, building);
    }
}

fn create_shallow_type(
    ast_type: &Type,
    ast_types_map: &HashMap<String, &Type>,
    building: &mut SystemContextBuilding,
) {
    if let Type::Composite { name, fields, .. } = &ast_type {
        let table_name = ast_type
            .get_annotation("table")
            .map(|a| a.params[0].as_string())
            .unwrap_or_else(|| name.clone());

        let columns = fields
            .iter()
            .flat_map(|ast_field| create_column(ast_field, &table_name, ast_types_map))
            .collect();

        let table = PhysicalTable {
            name: table_name.clone(),
            columns,
        };
        building.tables.add(&table_name, table);

        let model_type_name = ast_type.UNSAFE_name();
        building.types.add(
            &model_type_name,
            ModelType {
                name: model_type_name.to_owned(),
                kind: ModelTypeKind::Primitive,
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
                ModelType {
                    name: mutation_type_name.to_string(),
                    kind: ModelTypeKind::Primitive,
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
fn expand_type1(ast_type: &Type, building: &mut SystemContextBuilding) {
    let table_name = ast_type
        .get_annotation("table")
        .map(|a| a.params[0].as_string())
        .unwrap_or_else(|| ast_type.UNSAFE_name());

    let table_id = building.tables.get_id(&table_name).unwrap();

    let pk_query = building
        .queries
        .get_id(&query_builder::pk_query_name(&ast_type.UNSAFE_name()))
        .unwrap();
    let collection_query = building
        .queries
        .get_id(&query_builder::collection_query_name(
            &ast_type.UNSAFE_name(),
        ))
        .unwrap();

    let kind = ModelTypeKind::Composite {
        fields: vec![],
        table_id,
        pk_query,
        collection_query,
    };
    let existing_type_id = building.types.get_id(&ast_type.UNSAFE_name());

    building.types.values[existing_type_id.unwrap()].kind = kind;
}

fn expand_type2(ast_type: &Type, building: &mut SystemContextBuilding) {
    let existing_type_id = building.types.get_id(&ast_type.UNSAFE_name()).unwrap();
    let existing_type = &building.types[existing_type_id];

    if let ModelTypeKind::Composite {
        table_id,
        pk_query,
        collection_query,
        ..
    } = existing_type.kind
    {
        if let Type::Composite {
            fields: ast_fields, ..
        } = &ast_type
        {
            let model_fields: Vec<ModelField> = ast_fields
                .iter()
                .map(|ast_field| create_field(ast_field, table_id, building))
                .collect();

            let kind = ModelTypeKind::Composite {
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
                        ModelRelation::Pk { .. } => Some(field),
                        _ => None,
                    })
                    .collect();

                let existing_type_name = input_reference_type_name(&ast_type.UNSAFE_name());
                let existing_type_id = building.mutation_types.get_id(&existing_type_name).unwrap();

                building.mutation_types[existing_type_id].kind = ModelTypeKind::Composite {
                    fields: reference_type_fields,
                    table_id,
                    pk_query,
                    collection_query,
                }
            }

            {
                let input_type_fields = compute_input_fields(&model_fields, building, false);

                let existing_type_name = input_creation_type_name(&ast_type.UNSAFE_name());
                let existing_type_id = building.mutation_types.get_id(&existing_type_name).unwrap();

                building.mutation_types[existing_type_id].kind = ModelTypeKind::Composite {
                    fields: input_type_fields,
                    table_id,
                    pk_query,
                    collection_query,
                }
            }

            {
                let input_type_fields = compute_input_fields(&model_fields, building, true);

                let existing_type_name = input_update_type_name(&ast_type.UNSAFE_name());
                let existing_type_id = building.mutation_types.get_id(&existing_type_name).unwrap();

                building.mutation_types[existing_type_id].kind = ModelTypeKind::Composite {
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
    model_fields: &[ModelField],
    building: &SystemContextBuilding,
    force_optional_field_modifier: bool,
) -> Vec<ModelField> {
    model_fields
        .iter()
        .flat_map(|field| match &field.relation {
            ModelRelation::Pk { .. } => None,
            ModelRelation::Scalar { .. } => Some(ModelField {
                typ: field.typ.optional(),
                ..field.clone()
            }),
            ModelRelation::ManyToOne { .. } | ModelRelation::OneToMany { .. } => {
                let field_type_name = input_reference_type_name(&field.typ.type_name());
                let field_type_id = building.mutation_types.get_id(&field_type_name).unwrap();
                let field_plain_type = ModelFieldType::Plain {
                    type_name: field_type_name,
                    type_id: field_type_id,
                };
                let field_type = match field.typ {
                    ModelFieldType::Plain { .. } => field_plain_type,
                    ModelFieldType::Optional(_) => {
                        ModelFieldType::Optional(Box::new(field_plain_type))
                    }
                    ModelFieldType::List(_) => ModelFieldType::List(Box::new(field_plain_type)),
                };
                let field_type = if force_optional_field_modifier {
                    field_type.optional()
                } else {
                    field_type
                };
                Some(ModelField {
                    name: field.name.clone(),
                    typ: field_type,
                    relation: field.relation.clone(),
                })
            }
        })
        .collect()
}

fn create_field(
    ast_field: &TypedField,
    table_id: Id<PhysicalTable>,
    building: &SystemContextBuilding,
) -> ModelField {
    fn create_model_type(
        type_name: String,
        ast_field_type: &Type,
        building: &SystemContextBuilding,
    ) -> ModelFieldType {
        match ast_field_type {
            Type::Reference(_) | Type::Primitive(_) => ModelFieldType::Plain {
                type_name: type_name.clone(),
                type_id: building.types.get_id(&type_name).unwrap(),
            },
            Type::Optional(underlying) => ModelFieldType::Optional(Box::new(create_model_type(
                type_name, underlying, building,
            ))),
            Type::List(underlying) => {
                ModelFieldType::List(Box::new(create_model_type(type_name, underlying, building)))
            }
            o => panic!("Cannot create model type for type {:?}", o),
        }
    }

    let type_name = ast_field.typ.UNSAFE_name();
    ModelField {
        name: ast_field.name.to_owned(),
        typ: create_model_type(type_name, &ast_field.typ, building),
        relation: create_relation(&ast_field, table_id, building),
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
    ast_field: &TypedField,
    table_name: &str,
    ast_types_map: &HashMap<String, &Type>,
) -> Option<PhysicalColumn> {
    match ast_field.get_annotation("pk") {
        Some(_) => Some(PhysicalColumn {
            table_name: table_name.to_string(),
            column_name: ast_field
                .get_annotation("column")
                .map(|a| a.params[0].as_string())
                .unwrap_or_else(|| ast_field.name.clone()),
            typ: PhysicalColumnType::from_string(match ast_field.typ.as_primitive() {
                PrimitiveType::BOOLEAN => "Boolean",
                PrimitiveType::INTEGER => "Int",
                PrimitiveType::STRING => "String",
            }),
            is_pk: true,
            is_autoincrement: match ast_field.get_annotation("autoincrement") {
                Some(_) => {
                    assert!(ast_field.typ == Type::Primitive(PrimitiveType::INTEGER));
                    true
                }
                _ => false,
            },
            references: None,
        }),
        None { .. } => {
            match &ast_field.typ {
                Type::List(_) => None, // OneToMany, so the "many"-side type has the column

                Type::Primitive(_) => {
                    // Scalar type
                    Some(PhysicalColumn {
                        table_name: table_name.to_string(),
                        column_name: ast_field
                            .get_annotation("column")
                            .map(|a| a.params[0].as_string())
                            .unwrap_or_else(|| ast_field.name.clone()),
                        typ: PhysicalColumnType::from_string(match ast_field.typ.as_primitive() {
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
                    let other_type: &Type = ast_types_map[&o.UNSAFE_name()];
                    let other_type_pk_field = pk_field_of(other_type).unwrap();
                    let other_table_name = other_type
                        .get_annotation("table")
                        .map(|a| a.params[0].as_string())
                        .unwrap_or_else(|| o.UNSAFE_name());

                    Some(PhysicalColumn {
                        table_name: table_name.to_string(),
                        column_name: ast_field
                            .get_annotation("column")
                            .map(|a| a.params[0].as_string())
                            .unwrap_or_else(|| format!("{}_id", ast_field.name)),
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
                            column_name: ast_field
                                .get_annotation("column")
                                .map(|a| a.params[0].as_string())
                                .unwrap_or_else(|| ast_field.name.clone()),
                        }),
                    })
                }
            }
        }
    }
}

fn create_relation(
    ast_field: &TypedField,
    table_id: Id<PhysicalTable>,
    building: &SystemContextBuilding,
) -> ModelRelation {
    fn compute_column_name(column_name: &Option<String>, ast_field: &TypedField) -> String {
        column_name
            .clone()
            .unwrap_or_else(|| ast_field.name.clone())
    }

    fn compute_column_id(
        table: &PhysicalTable,
        table_id: Id<PhysicalTable>,
        column_name: &Option<String>,
        ast_field: &TypedField,
    ) -> Option<ColumnId> {
        let column_name = compute_column_name(column_name, ast_field);

        table
            .column_index(&column_name)
            .map(|index| ColumnId::new(table_id, index))
    }

    let table = &building.tables[table_id];

    match ast_field.get_annotation("pk") {
        Some(_) => {
            let column_id = compute_column_id(
                table,
                table_id,
                &ast_field
                    .get_annotation("column")
                    .map(|a| a.params[0].as_string()),
                ast_field,
            );
            ModelRelation::Pk {
                column_id: column_id.unwrap(),
            }
        }
        None => {
            match &ast_field.typ {
                // Not primitive
                Type::List(i) => {
                    let other_type_id = building.types.get_id(&i.UNSAFE_name()).unwrap();
                    let other_type = &building.types[other_type_id];
                    let other_table_id = other_type.table_id().unwrap();
                    let other_table = &building.tables[other_table_id];
                    let other_type_column_id = compute_column_id(
                        other_table,
                        other_table_id,
                        &ast_field
                            .get_annotation("column")
                            .map(|a| a.params[0].as_string()),
                        ast_field,
                    )
                    .unwrap();

                    ModelRelation::OneToMany {
                        other_type_column_id,
                        other_type_id,
                    }
                }

                Type::Primitive(_) => {
                    // Primitive
                    let column_id = compute_column_id(
                        table,
                        table_id,
                        &ast_field
                            .get_annotation("column")
                            .map(|a| a.params[0].as_string()),
                        ast_field,
                    );
                    ModelRelation::Scalar {
                        column_id: column_id.unwrap(),
                    }
                }

                o => {
                    let optional = matches!(o, Type::Optional(_));
                    // ManyToOne
                    let column_id = compute_column_id(
                        table,
                        table_id,
                        &ast_field
                            .get_annotation("column")
                            .map(|a| a.params[0].as_string()),
                        ast_field,
                    );
                    let other_type_id =
                        building.types.get_id(&ast_field.typ.UNSAFE_name()).unwrap();
                    ModelRelation::ManyToOne {
                        column_id: column_id.unwrap(),
                        other_type_id,
                        optional,
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
