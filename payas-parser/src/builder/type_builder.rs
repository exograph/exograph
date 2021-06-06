use core::panic;

use id_arena::Id;
use payas_model::{
    model::{column_id::ColumnId, mapped_arena::MappedArena, relation::GqlRelation, GqlFieldType},
    sql::{
        column::{ColumnReferece, PhysicalColumn},
        PhysicalTable,
    },
};

use super::query_builder;
use super::system_builder::SystemContextBuilding;

use crate::typechecker::{CompositeType, PrimitiveType, Type, TypedField};

use payas_model::model::{GqlField, GqlType, GqlTypeKind};

pub fn build_shallow(models: &MappedArena<Type>, building: &mut SystemContextBuilding) {
    for (_, model_type) in models.iter() {
        create_shallow_type(model_type, building);
    }
}

pub fn build_expanded(env: &MappedArena<Type>, building: &mut SystemContextBuilding) {
    for (_, model_type) in env.iter() {
        if let Type::Composite(c) = &model_type {
            expand_type1(c, building, env);
        }
    }
    for (_, model_type) in env.iter() {
        if let Type::Composite(c) = &model_type {
            expand_type2(c, building, env);
        }
    }
}

fn create_shallow_type(model_type: &Type, building: &mut SystemContextBuilding) {
    let (type_name, is_composite) = match model_type {
        Type::Composite(c) => (c.name.clone(), true),
        Type::Primitive(pt) => (pt.name().to_string(), false),
        _ => panic!(""),
    };

    // Mark every type as Primitive, since other types that may be referred haven't been processed yet
    building.types.add(
        &type_name,
        GqlType {
            name: type_name.to_string(),
            kind: GqlTypeKind::Primitive,
            is_input: false,
        },
    );

    if is_composite {
        let mutation_type_names = [
            input_creation_type_name(&type_name),
            input_update_type_name(&type_name),
            input_reference_type_name(&type_name),
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
    }
}

fn compute_table_name(typ: &CompositeType) -> String {
    typ.get_annotation("table")
        .map(|a| a.params[0].as_string())
        .unwrap_or_else(|| typ.name.clone())
}

fn compute_column_name(field: &TypedField, reference: bool) -> String {
    field
        .get_annotation("column")
        .map(|a| a.params[0].as_string())
        .unwrap_or_else(|| {
            if reference {
                format!("{}_id", field.name)
            } else {
                field.name.clone()
            }
        })
}

// Expand type except for model fields. This allows types to become `Composite` and `table_id` for any type
// can be accessed when building fields
fn expand_type1(
    model_type: &CompositeType,
    building: &mut SystemContextBuilding,
    env: &MappedArena<Type>,
) {
    let table_name = compute_table_name(model_type);

    let columns = model_type
        .fields
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
        .get_id(&query_builder::pk_query_name(&model_type.name))
        .unwrap();

    let collection_query = building
        .queries
        .get_id(&query_builder::collection_query_name(&model_type.name))
        .unwrap();

    let kind = GqlTypeKind::Composite {
        fields: vec![],
        table_id,
        pk_query,
        collection_query,
    };
    let existing_type_id = building.types.get_id(&model_type.name);

    building.types.values[existing_type_id.unwrap()].kind = kind;
}

fn expand_type2(
    model_type: &CompositeType,
    building: &mut SystemContextBuilding,
    env: &MappedArena<Type>,
) {
    let existing_type_id = building.types.get_id(&model_type.name).unwrap();
    let existing_type = &building.types[existing_type_id];

    if let GqlTypeKind::Composite {
        table_id,
        pk_query,
        collection_query,
        ..
    } = existing_type.kind
    {
        let model_fields: Vec<GqlField> = model_type
            .fields
            .iter()
            .map(|field| create_field(model_type, field, table_id, building, env))
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

            let existing_type_name = input_reference_type_name(model_type.name.as_str());
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

            let existing_type_name = input_creation_type_name(model_type.name.as_str());
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

            let existing_type_name = input_update_type_name(model_type.name.as_str());
            let existing_type_id = building.mutation_types.get_id(&existing_type_name).unwrap();

            building.mutation_types[existing_type_id].kind = GqlTypeKind::Composite {
                fields: input_type_fields,
                table_id,
                pk_query,
                collection_query,
            }
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
                    GqlFieldType::Optional(_) => GqlFieldType::Optional(Box::new(field_plain_type)),
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
    model_type: &CompositeType,
    field: &TypedField,
    table_id: Id<PhysicalTable>,
    building: &SystemContextBuilding,
    env: &MappedArena<Type>,
) -> GqlField {
    fn create_field_type(field_type: &Type, building: &SystemContextBuilding) -> GqlFieldType {
        match field_type {
            Type::Reference(r) => GqlFieldType::Reference {
                type_name: r.clone(),
                type_id: building.types.get_id(&r).unwrap(),
            },
            Type::Optional(underlying) => {
                GqlFieldType::Optional(Box::new(create_field_type(underlying, building)))
            }
            Type::List(underlying) => {
                GqlFieldType::List(Box::new(create_field_type(underlying, building)))
            }
            o => panic!("Cannot create model type for type {:?}", o),
        }
    }

    GqlField {
        name: field.name.to_owned(),
        typ: create_field_type(&field.typ, building),
        relation: create_relation(model_type, field, table_id, building, env),
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
            column_name: compute_column_name(field, false),
            typ: field.typ.deref(env).as_primitive().to_column_type(),
            is_pk: true,
            is_autoincrement: match field.get_annotation("autoincrement") {
                Some(_) => {
                    assert!(field.typ.deref(env) == Type::Primitive(PrimitiveType::Int));
                    true
                }
                _ => false,
            },
            references: None,
        }),
        None { .. } => {
            match &field.typ.deref(env) {
                Type::List(_) => None, // OneToMany, so the "many"-side type has the column

                Type::Primitive(p) => {
                    // Scalar type
                    Some(PhysicalColumn {
                        table_name: table_name.to_string(),
                        column_name: compute_column_name(field, false),
                        typ: p.to_column_type(),
                        is_pk: false,
                        is_autoincrement: false,
                        references: None,
                    })
                }

                other_type => {
                    // ManyToOne
                    let inner_composite = other_type.inner_composite(env);
                    let other_type_pk_field = inner_composite
                        .fields
                        .iter()
                        .find(|f| f.get_annotation("pk").is_some())
                        .unwrap();
                    let other_table_name = compute_table_name(inner_composite);

                    Some(PhysicalColumn {
                        table_name: table_name.to_string(),
                        column_name: compute_column_name(field, true),
                        typ: other_type_pk_field
                            .typ
                            .deref(env)
                            .as_primitive()
                            .to_column_type(),
                        is_pk: false,
                        is_autoincrement: false,
                        references: Some(ColumnReferece {
                            table_name: other_table_name,
                            column_name: compute_column_name(other_type_pk_field, false),
                        }),
                    })
                }
            }
        }
    }
}

fn create_relation(
    model_type: &CompositeType,
    field: &TypedField,
    table_id: Id<PhysicalTable>,
    building: &SystemContextBuilding,
    env: &MappedArena<Type>,
) -> GqlRelation {
    fn compute_column_id(
        table: &PhysicalTable,
        table_id: Id<PhysicalTable>,
        field: &TypedField,
        reference: bool,
    ) -> Option<ColumnId> {
        let column_name = compute_column_name(field, reference);

        table
            .column_index(&column_name)
            .map(|index| ColumnId::new(table_id, index))
    }

    let table = &building.tables[table_id];

    match field.get_annotation("pk") {
        Some(_) => {
            let column_id = compute_column_id(table, table_id, field, false);
            GqlRelation::Pk {
                column_id: column_id.unwrap(),
            }
        }
        None => {
            match &field.typ.deref(env) {
                // Not primitive
                Type::List(i) => {
                    let other_type_id = building
                        .types
                        .get_id(i.as_ref().inner_composite(env).name.as_str())
                        .unwrap();
                    let other_type = &building.types[other_type_id];
                    let other_table_id = other_type.table_id().unwrap();
                    let other_table = &building.tables[other_table_id];

                    let column_name = field
                        .get_annotation("column")
                        .map(|a| a.params[0].as_string())
                        .unwrap_or_else(|| format!("{}_id", model_type.name.to_ascii_lowercase()));
                    let other_type_column_id = other_table
                        .column_index(&column_name)
                        .map(|index| ColumnId::new(other_table_id, index))
                        .unwrap();

                    GqlRelation::OneToMany {
                        other_type_column_id,
                        other_type_id,
                    }
                }

                Type::Primitive(_) => {
                    // Primitive
                    let column_id = compute_column_id(table, table_id, field, false);
                    GqlRelation::Scalar {
                        column_id: column_id.unwrap(),
                    }
                }

                o => {
                    // ManyToOne
                    let column_id = compute_column_id(table, table_id, field, true);
                    let other_type_id = building
                        .types
                        .get_id(o.inner_composite(env).name.as_str())
                        .unwrap();
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
