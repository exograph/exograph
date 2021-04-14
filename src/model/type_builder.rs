use id_arena::Id;

use super::column_id::ColumnId;
use super::relation::ModelRelation;
use crate::model::system_context::SystemContextBuilding;
use crate::sql::table::PhysicalTable;
use crate::{model::ast::ast_types::*, sql::column::PhysicalColumn};

use super::types::{ModelField, ModelType, ModelTypeKind, ModelTypeModifier};

const PRIMITIVE_TYPE_NAMES: [&str; 2] = ["Int", "String"]; // TODO: Expand the list

pub fn build(ast_types: &[AstType], building: &mut SystemContextBuilding) {
    for type_name in PRIMITIVE_TYPE_NAMES.iter() {
        let typ = ModelType {
            name: type_name.to_string(),
            kind: ModelTypeKind::Primitive,
        };
        building.types.add(type_name, typ);
    }

    for ast_type in ast_types {
        let shallow_type = create_shallow_type(ast_type, building);
        building.types.add(&shallow_type.name.clone(), shallow_type);
    }

    for ast_type in ast_types {
        let existing_type_id = building.types.get_id(&ast_type.name);
        let existing_type = existing_type_id.and_then(|id| building.types.get_by_id(id));

        match existing_type {
            Some(existing_type) => {
                let new_kind = expand_type(existing_type, ast_type, &building);
                building.update_type(existing_type_id.unwrap(), new_kind);
            }
            None => panic!(""),
        }
    }
}

fn create_shallow_type(ast_type: &AstType, building: &mut SystemContextBuilding) -> ModelType {
    let kind = match &ast_type.kind {
        AstTypeKind::Primitive => ModelTypeKind::Primitive,
        AstTypeKind::Composite {
            fields: ast_field,
            table_name: ast_table_name,
        } => {
            let table_name = ast_table_name.clone().unwrap_or(ast_type.name.clone());
            let columns = ast_field
                .iter()
                .flat_map(|ast_field| create_column(ast_field, &table_name))
                .collect();

            let table = PhysicalTable {
                name: table_name.clone(),
                columns,
            };
            let table_id = building.tables.add(&table_name, table);
            ModelTypeKind::Composite {
                fields: vec![],
                table_id,
            }
        }
    };

    ModelType {
        name: ast_type.name.to_string(),
        kind,
    }
}

fn expand_type(
    existing_type: &ModelType,
    ast_type: &AstType,
    building: &SystemContextBuilding,
) -> ModelTypeKind {
    match (&ast_type.kind, &existing_type.kind) {
        (AstTypeKind::Primitive, ModelTypeKind::Primitive) => ModelTypeKind::Primitive,
        (
            AstTypeKind::Composite {
                fields: ast_fields, ..
            },
            ModelTypeKind::Composite {
                fields: _,
                table_id,
            },
        ) => {
            let model_fields = ast_fields
                .iter()
                .map(|ast_field| create_field(ast_field, *table_id, building))
                .collect();

            ModelTypeKind::Composite {
                fields: model_fields,
                table_id: *table_id,
            }
        }
        _ => panic!(""),
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
        AstRelation::OneToMany { column_name, .. } => Some(PhysicalColumn {
            table_name: table_name.to_string(),
            column_name: column_name.clone().unwrap_or(ast_field.name.clone()),
        }),
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
        table: Option<&PhysicalTable>,
        table_id: Id<PhysicalTable>,
        column_name: &Option<String>,
        ast_field: &AstField,
    ) -> Option<ColumnId> {
        let column_name = compute_column_name(column_name, ast_field);
        table.and_then(|table| {
            table
                .column_index(&column_name)
                .map(|index| ColumnId::new(table_id, index))
        })
    }

    let table = building.tables.get_by_id(table_id);

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
            column_name,
            other_type_name,
            optional,
        } => {
            let column_id = compute_column_id(table, table_id, column_name, ast_field);
            let other_type_id = building.types.get_id(other_type_name).unwrap();
            ModelRelation::OneToMany {
                column_id: column_id.unwrap(),
                other_type_id,
                optional: *optional,
            }
        }
    }
}
