use id_arena::Id;
use payas_model::{
    model::{column_id::ColumnId, mapped_arena::MappedArena, relation::GqlRelation, GqlFieldType},
    sql::{
        column::{ColumnReferece, PhysicalColumn},
        PhysicalTable,
    },
};

use super::{
    query_builder,
    resolved_builder::{ResolvedField, ResolvedFieldType, ResolvedType},
};
use super::{resolved_builder::ResolvedCompositeType, system_builder::SystemContextBuilding};

use crate::typechecker::PrimitiveType;

use payas_model::model::{GqlField, GqlType, GqlTypeKind};

pub fn build_shallow(models: &MappedArena<ResolvedType>, building: &mut SystemContextBuilding) {
    for (_, model_type) in models.iter() {
        create_shallow_type(model_type, building);
    }
}

pub fn build_expanded(env: &MappedArena<ResolvedType>, building: &mut SystemContextBuilding) {
    for (_, model_type) in env.iter() {
        if let ResolvedType::Composite(c) = &model_type {
            expand_type1(c, building, env);
        }
    }
    for (_, model_type) in env.iter() {
        if let ResolvedType::Composite(c) = &model_type {
            expand_type2(c, building, env);
        }
    }
}

fn create_shallow_type(resolved_type: &ResolvedType, building: &mut SystemContextBuilding) {
    let (type_name, is_composite) = match resolved_type {
        ResolvedType::Composite(ResolvedCompositeType { name, .. }) => (name.clone(), true),
        ResolvedType::Primitive(pt) => (pt.name().to_string(), false),
    };

    // Mark every type as Primitive, since other types that may be referred haven't been processed yet
    // and we haven't build query and mutation types either
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

// Expand type except for model fields. Specifically, set the table and *_query members, but leave fields as an empty vector.
// This allows types to become `Composite` and `table_id` for any type can be accessed when building fields
fn expand_type1(
    resolved_type: &ResolvedCompositeType,
    building: &mut SystemContextBuilding,
    env: &MappedArena<ResolvedType>,
) {
    let table_name = resolved_type.table_name.clone();

    let columns = resolved_type
        .fields
        .iter()
        .flat_map(|field| create_column(field, &table_name, env))
        .collect();

    let table = PhysicalTable {
        name: resolved_type.table_name.clone(),
        columns,
    };

    let table_id = building.tables.add(&table_name, table);

    let pk_query = building
        .queries
        .get_id(&query_builder::pk_query_name(&resolved_type.name))
        .unwrap();

    let collection_query = building
        .queries
        .get_id(&query_builder::collection_query_name(&resolved_type.name))
        .unwrap();

    let kind = GqlTypeKind::Composite {
        fields: vec![],
        table_id,
        pk_query,
        collection_query,
    };
    let existing_type_id = building.types.get_id(&resolved_type.name);

    building.types.values[existing_type_id.unwrap()].kind = kind;
}

fn expand_type2(
    resolved_type: &ResolvedCompositeType,
    building: &mut SystemContextBuilding,
    env: &MappedArena<ResolvedType>,
) {
    let existing_type_id = building.types.get_id(&resolved_type.name).unwrap();
    let existing_type = &building.types[existing_type_id];

    if let GqlTypeKind::Composite {
        table_id,
        pk_query,
        collection_query,
        ..
    } = existing_type.kind
    {
        let model_fields: Vec<GqlField> = resolved_type
            .fields
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

            let existing_type_name = input_reference_type_name(resolved_type.name.as_str());
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

            let existing_type_name = input_creation_type_name(resolved_type.name.as_str());
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

            let existing_type_name = input_update_type_name(resolved_type.name.as_str());
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
    field: &ResolvedField,
    table_id: Id<PhysicalTable>,
    building: &SystemContextBuilding,
    env: &MappedArena<ResolvedType>,
) -> GqlField {
    fn create_field_type(
        field_type: &ResolvedFieldType,
        building: &SystemContextBuilding,
    ) -> GqlFieldType {
        match field_type {
            ResolvedFieldType::Plain(r) => GqlFieldType::Reference {
                type_name: r.clone(),
                type_id: building.types.get_id(&r).unwrap(),
            },
            ResolvedFieldType::Optional(underlying) => {
                GqlFieldType::Optional(Box::new(create_field_type(underlying, building)))
            }
            ResolvedFieldType::List(underlying) => {
                GqlFieldType::List(Box::new(create_field_type(underlying, building)))
            }
        }
    }

    GqlField {
        name: field.name.to_owned(),
        typ: create_field_type(&field.typ, building),
        relation: create_relation(field, table_id, building, env),
    }
}

fn create_column(
    field: &ResolvedField,
    table_name: &str,
    env: &MappedArena<ResolvedType>,
) -> Option<PhysicalColumn> {
    match &field.typ {
        ResolvedFieldType::Plain(type_name) => {
            let field_type = env.get_by_key(&type_name).unwrap();

            match field_type {
                ResolvedType::Primitive(pt) => Some(PhysicalColumn {
                    table_name: table_name.to_string(),
                    column_name: field.column_name.clone(),
                    typ: pt.to_column_type(),
                    is_pk: field.is_pk,
                    is_autoincrement: if field.is_autoincrement {
                        assert!(
                            field.typ.deref(env) == &ResolvedType::Primitive(PrimitiveType::Int)
                        );
                        true
                    } else {
                        false
                    },
                    references: None,
                }),
                ResolvedType::Composite(ct) => {
                    let pk_field = ct.pk_field().unwrap();
                    Some(PhysicalColumn {
                        table_name: ct.table_name.to_string(),
                        column_name: field.column_name.clone(),
                        typ: pk_field.typ.deref(env).as_primitive().to_column_type(),
                        is_pk: false,
                        is_autoincrement: false,
                        references: Some(ColumnReferece {
                            table_name: ct.table_name.clone(),
                            column_name: field.column_name.clone(),
                        }),
                    })
                }
            }
        }
        ResolvedFieldType::Optional(_) => {
            todo!()
        }
        ResolvedFieldType::List(_) => {
            // OneToMany, so the other side has the associated column
            None
        }
    }
}

fn create_relation(
    field: &ResolvedField,
    table_id: Id<PhysicalTable>,
    building: &SystemContextBuilding,
    env: &MappedArena<ResolvedType>,
) -> GqlRelation {
    fn compute_column_id(
        table: &PhysicalTable,
        table_id: Id<PhysicalTable>,
        field: &ResolvedField,
    ) -> Option<ColumnId> {
        let column_name = field.column_name.clone();

        table
            .column_index(&column_name)
            .map(|index| ColumnId::new(table_id, index))
    }

    let table = &building.tables[table_id];

    if field.is_pk {
        let column_id = compute_column_id(table, table_id, field);
        GqlRelation::Pk {
            column_id: column_id.unwrap(),
        }
    } else {
        match &field.typ {
            ResolvedFieldType::List(underlying) => {
                let field_type = underlying.deref(env).as_composite();

                let other_type_id = building.types.get_id(field_type.name.as_str()).unwrap();
                let other_type = &building.types[other_type_id];
                let other_table_id = other_type.table_id().unwrap();
                let other_table = &building.tables[other_table_id];

                let column_name = field.column_name.clone();
                let other_type_column_id = other_table
                    .column_index(&column_name)
                    .map(|index| ColumnId::new(other_table_id, index))
                    .unwrap();

                GqlRelation::OneToMany {
                    other_type_column_id,
                    other_type_id,
                }
            }

            ResolvedFieldType::Plain(type_name) => {
                let field_type = env.get_by_key(&type_name).unwrap();

                match field_type {
                    ResolvedType::Primitive(_) => {
                        let column_id = compute_column_id(table, table_id, field);
                        GqlRelation::Scalar {
                            column_id: column_id.unwrap(),
                        }
                    }
                    ResolvedType::Composite(ct) => {
                        // ManyToOne
                        let column_id = compute_column_id(table, table_id, field);
                        let other_type_id = building.types.get_id(&ct.name).unwrap();
                        GqlRelation::ManyToOne {
                            column_id: column_id.unwrap(),
                            other_type_id,
                            optional: matches!(field.typ, ResolvedFieldType::Optional(_)),
                        }
                    }
                }
            }
            ResolvedFieldType::Optional(_) => todo!(),
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
