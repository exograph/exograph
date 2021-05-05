use id_arena::Id;

use super::relation::ModelRelation;
use super::{column_id::ColumnId, query_builder};
use crate::model::system_context::SystemContextBuilding;
use crate::sql::table::PhysicalTable;
use crate::{model::ast::ast_types::*, sql::column::PhysicalColumn};

use super::types::{ModelField, ModelType, ModelTypeKind, ModelTypeModifier};

pub const PRIMITIVE_TYPE_NAMES: [&str; 2] = ["Int", "String"]; // TODO: Expand the list

pub fn build_shallow(ast_types: &[AstType], building: &mut SystemContextBuilding) {
    for type_name in PRIMITIVE_TYPE_NAMES.iter() {
        let typ = ModelType {
            name: type_name.to_string(),
            kind: ModelTypeKind::Primitive,
            is_input: false,
        };
        building.types.add(type_name, typ);
    }

    for ast_type in ast_types {
        create_shallow_type(ast_type, building);
    }
}

pub fn build_expanded(ast_types: &[AstType], building: &mut SystemContextBuilding) {
    for ast_type in ast_types {
        expand_type1(ast_type, building);
    }
    for ast_type in ast_types {
        expand_type2(ast_type, building);
    }
}

fn create_shallow_type(ast_type: &AstType, building: &mut SystemContextBuilding) {
    if let AstTypeKind::Composite {
        fields: ast_fields,
        table_name: ast_table_name,
    } = &ast_type.kind
    {
        let table_name = ast_table_name.clone().unwrap_or(ast_type.name.clone());
        let columns = ast_fields
            .iter()
            .flat_map(|ast_field| create_column(ast_field, &table_name))
            .collect();

        let table = PhysicalTable {
            name: table_name.clone(),
            columns,
        };
        building.tables.add(&table_name, table);

        let model_type_name = ast_type.name.to_owned();
        building.types.add(
            &model_type_name.to_owned(),
            ModelType {
                name: model_type_name.to_owned().to_string(),
                kind: ModelTypeKind::Primitive,
                is_input: false,
            },
        );

        let mutation_type_names = [
            input_type_name(&model_type_name),
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
    }
}

// Expand type except for model fields. This allows types to become `Composite` and `table_id` for any type
// can be accessed when building fields
fn expand_type1(ast_type: &AstType, building: &mut SystemContextBuilding) {
    if let AstTypeKind::Composite {
        table_name: ast_table_name,
        ..
    } = &ast_type.kind
    {
        let table_name = ast_table_name.clone().unwrap_or(ast_type.name.clone());
        let table_id = building.tables.get_id(&table_name).unwrap();

        let pk_query = building
            .queries
            .get_id(&query_builder::pk_query_name(&ast_type.name))
            .unwrap();
        let collection_query = building
            .queries
            .get_id(&query_builder::collection_query_name(&ast_type.name))
            .unwrap();

        let kind = ModelTypeKind::Composite {
            fields: vec![],
            table_id,
            pk_query,
            collection_query,
        };
        let existing_type_id = building.types.get_id(&ast_type.name);

        building.types.values[existing_type_id.unwrap()].kind = kind;
    }
}

fn expand_type2(ast_type: &AstType, building: &mut SystemContextBuilding) {
    let existing_type_id = building.types.get_id(&ast_type.name).unwrap();
    let existing_type = &building.types[existing_type_id];

    if let ModelTypeKind::Composite {
        table_id,
        pk_query,
        collection_query,
        ..
    } = existing_type.kind
    {
        if let AstTypeKind::Composite {
            fields: ast_fields, ..
        } = &ast_type.kind
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

                let existing_type_name = input_reference_type_name(&ast_type.name);
                let existing_type_id = building.mutation_types.get_id(&existing_type_name).unwrap();

                building.mutation_types[existing_type_id].kind = ModelTypeKind::Composite {
                    fields: reference_type_fields,
                    table_id,
                    pk_query,
                    collection_query,
                }
            }

            {
                let input_type_fields = model_fields
                    .into_iter()
                    .flat_map(|field| match &field.relation {
                        ModelRelation::Pk { .. } => None,
                        ModelRelation::Scalar { .. } => Some(field),
                        ModelRelation::ManyToOne { .. } => {
                            let field_type_name = input_reference_type_name(&field.type_name);
                            let field_type_id =
                                building.mutation_types.get_id(&field_type_name).unwrap();
                            let new_field = ModelField {
                                name: field.name,
                                type_id: field_type_id,
                                type_name: field_type_name,
                                type_modifier: field.type_modifier,
                                relation: field.relation,
                            };
                            Some(new_field)
                        }
                        ModelRelation::OneToMany { .. } => {
                            let field_type_name = input_reference_type_name(&field.type_name);
                            let field_type_id =
                                building.mutation_types.get_id(&field_type_name).unwrap();
                            let new_field = ModelField {
                                name: field.name,
                                type_id: field_type_id,
                                type_name: field_type_name,
                                type_modifier: ModelTypeModifier::List,
                                relation: field.relation,
                            };
                            Some(new_field)
                        }
                    })
                    .collect();

                let existing_type_name = input_type_name(&ast_type.name);
                let existing_type_id = building.mutation_types.get_id(&existing_type_name).unwrap();

                building.mutation_types[existing_type_id].kind = ModelTypeKind::Composite {
                    fields: input_type_fields,
                    table_id,
                    pk_query,
                    collection_query,
                }
            }
        }
    }
}

fn create_field(
    ast_field: &AstField,
    table_id: Id<PhysicalTable>,
    building: &SystemContextBuilding,
) -> ModelField {
    fn create_type_modifier(ast_type_modifier: &AstTypeModifier) -> ModelTypeModifier {
        match ast_type_modifier {
            AstTypeModifier::Optional => ModelTypeModifier::Optional,
            AstTypeModifier::NonNull => ModelTypeModifier::NonNull,
            AstTypeModifier::List => ModelTypeModifier::List,
        }
    }

    ModelField {
        name: ast_field.name.to_owned(),
        type_id: building.types.get_id(&ast_field.type_name).unwrap(),
        type_name: ast_field.type_name.to_owned(),
        type_modifier: create_type_modifier(&ast_field.type_modifier),
        relation: create_relation(&ast_field, table_id, building),
    }
}

fn create_column(ast_field: &AstField, table_name: &str) -> Option<PhysicalColumn> {
    match &ast_field.relation {
        AstRelation::Pk { column_name } | AstRelation::Scalar { column_name } => {
            Some(PhysicalColumn {
                table_name: table_name.to_string(),
                column_name: column_name.clone().unwrap_or(ast_field.name.clone()),
            })
        }
        AstRelation::ManyToOne { column_name, .. } => Some(PhysicalColumn {
            table_name: table_name.to_string(),
            column_name: column_name.clone().unwrap_or(ast_field.name.clone()),
        }),
        AstRelation::OneToMany { .. } => None, // TODO: Add this column to the other table (needed when the other side doesn't include a corresponding ManyToOne)
    }
}

fn create_relation(
    ast_field: &AstField,
    table_id: Id<PhysicalTable>,
    building: &SystemContextBuilding,
) -> ModelRelation {
    fn compute_column_name(column_name: &Option<String>, ast_field: &AstField) -> String {
        column_name.clone().unwrap_or(ast_field.name.clone())
    }

    fn compute_column_id(
        table: &PhysicalTable,
        table_id: Id<PhysicalTable>,
        column_name: &Option<String>,
        ast_field: &AstField,
    ) -> Option<ColumnId> {
        let column_name = compute_column_name(column_name, ast_field);

        table
            .column_index(&column_name)
            .map(|index| ColumnId::new(table_id, index))
    }

    let table = &building.tables[table_id];

    match &ast_field.relation {
        AstRelation::Pk { column_name } => {
            let column_id = compute_column_id(table, table_id, column_name, ast_field);
            ModelRelation::Pk {
                column_id: column_id.unwrap(),
            }
        }
        AstRelation::Scalar { column_name } => {
            let column_id = compute_column_id(table, table_id, column_name, ast_field);
            ModelRelation::Scalar {
                column_id: column_id.unwrap(),
            }
        }
        AstRelation::ManyToOne {
            column_name,
            other_type_name,
            optional,
        } => {
            let column_id = compute_column_id(table, table_id, column_name, ast_field);
            let other_type_id = building.types.get_id(other_type_name).unwrap();
            ModelRelation::ManyToOne {
                column_id: column_id.unwrap(),
                other_type_id,
                optional: *optional,
            }
        }
        AstRelation::OneToMany {
            other_type_column_name,
            other_type_name,
        } => {
            let other_type_id = building.types.get_id(other_type_name).unwrap();
            let other_type = &building.types[other_type_id];
            let other_table_id = other_type.table_id().unwrap();
            let other_table = &building.tables[other_table_id];
            let other_type_column_id = compute_column_id(
                other_table,
                other_table_id,
                other_type_column_name,
                ast_field,
            )
            .unwrap();

            ModelRelation::OneToMany {
                other_type_column_id,
                other_type_id,
            }
        }
    }
}

pub fn input_type_name(model_type_name: &str) -> String {
    format!("{}Input", model_type_name)
}

pub fn input_reference_type_name(model_type_name: &str) -> String {
    format!("{}ReferenceInput", model_type_name)
}
