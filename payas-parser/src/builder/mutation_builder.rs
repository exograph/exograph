//! Build mutation input types (<Type>CreationInput, <Type>UpdateInput, <Type>ReferenceInput) and
//! mutations (create<Type>, update<Type>, and delete<Type> as well as their plural versions)

use std::collections::HashSet;

use id_arena::Id;
use payas_model::model::access::Access;
use payas_model::model::mapped_arena::MappedArena;
use payas_model::model::naming::{ToGqlMutationNames, ToGqlTypeNames};
use payas_model::model::relation::GqlRelation;
use payas_model::model::{operation::Mutation, types::GqlType};
use payas_model::model::{GqlCompositeTypeKind, GqlField, GqlFieldType, GqlTypeKind};

use crate::builder::query_builder;

use payas_model::model::{
    operation::{CreateDataParameter, MutationKind, OperationReturnType, UpdateDataParameter},
    types::GqlTypeModifier,
};

use super::resolved_builder::{ResolvedCompositeType, ResolvedFieldType, ResolvedType};
use super::system_builder::SystemContextBuilding;

/// Build shallow mutaiton input types
pub fn build_shallow(models: &MappedArena<ResolvedType>, building: &mut SystemContextBuilding) {
    for (_, model_type) in models.iter() {
        create_shallow_type(model_type, models, building);
    }
}

/// Expand the mutation input types as well as build the mutation
pub fn build_expanded(building: &mut SystemContextBuilding) {
    for (_, model_type) in building.types.iter() {
        if let GqlTypeKind::Composite { .. } = &model_type.kind {
            for (existing_id, expanded_kind) in expanded_mutation_types(model_type, building) {
                building.mutation_types[existing_id].kind = expanded_kind;
            }
        }
    }
    for (_, model_type) in building.types.iter() {
        if let GqlTypeKind::Composite { .. } = &model_type.kind {
            let model_type_id = building.types.get_id(model_type.name.as_str()).unwrap();

            let mutations = build_create_mutation(model_type_id, model_type, building)
                .into_iter()
                .chain(
                    build_delete_mutations(model_type_id, model_type, building)
                        .into_iter()
                        .chain(
                            build_update_mutations(model_type_id, model_type, building).into_iter(),
                        ),
                );

            for mutation in mutations {
                building.mutations.add(&mutation.name.to_owned(), mutation);
            }
        }
    }
}

fn create_shallow_type(
    resolved_type: &ResolvedType,
    models: &MappedArena<ResolvedType>,
    building: &mut SystemContextBuilding,
) {
    if let ResolvedType::Composite(c) = resolved_type {
        let mutation_type_names = [c.creation_type(), c.update_type(), c.reference_type()];

        let nested_creation_type_names: Vec<_> = c
            .fields
            .iter()
            .filter_map(|field| {
                let field_type = field.typ.deref(models);
                // Create a nested input data type only if it refers to a many side
                // So for Venue <-> [Concert] case, create only ConcertCreationInputFromVenue
                if let ResolvedFieldType::List(_) = field.typ {
                    if let ResolvedType::Composite(ResolvedCompositeType { name, .. }) = field_type
                    {
                        Some(input_creation_type_name(name, Some(&c.name)))
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect();
        // TODO: Check why do we need to collect() above in order to chain() in the next line

        for mutation_type_name in mutation_type_names
            .iter()
            .chain(nested_creation_type_names.iter())
        {
            building.mutation_types.add(
                mutation_type_name,
                GqlType {
                    name: mutation_type_name.to_string(),
                    plural_name: "".to_string(), // unused
                    kind: GqlTypeKind::Primitive,
                    is_input: true,
                },
            );
        }
    }
}

fn build_create_mutation(
    model_type_id: Id<GqlType>,
    model_type: &GqlType,
    building: &SystemContextBuilding,
) -> Vec<Mutation> {
    let data_param_type_name = model_type.creation_type();
    let data_param_type_id = building
        .mutation_types
        .get_id(&data_param_type_name)
        .unwrap();

    let single_create = Mutation {
        name: model_type.pk_create(),
        kind: MutationKind::Create(CreateDataParameter {
            name: "data".to_string(),
            type_name: data_param_type_name.clone(),
            type_id: data_param_type_id,
            array_input: false,
        }),
        return_type: OperationReturnType {
            type_id: model_type_id,
            type_name: model_type.name.clone(),
            type_modifier: GqlTypeModifier::Optional,
        },
    };

    let multi_create = Mutation {
        name: model_type.collection_create(),
        kind: MutationKind::Create(CreateDataParameter {
            name: "data".to_string(),
            type_name: data_param_type_name,
            type_id: data_param_type_id,
            array_input: true,
        }),
        return_type: OperationReturnType {
            type_id: model_type_id,
            type_name: model_type.name.clone(),
            type_modifier: GqlTypeModifier::List,
        },
    };

    vec![single_create, multi_create]
}

fn build_delete_mutations(
    model_type_id: Id<GqlType>,
    model_type: &GqlType,
    building: &SystemContextBuilding,
) -> Vec<Mutation> {
    let by_pk_delete = Mutation {
        name: model_type.pk_delete(),
        kind: MutationKind::Delete(query_builder::pk_predicate_param(model_type, building)),
        return_type: OperationReturnType {
            type_id: model_type_id,
            type_name: model_type.name.clone(),
            type_modifier: GqlTypeModifier::Optional,
        },
    };

    let by_predicate_delete = Mutation {
        name: model_type.collection_delete(),
        kind: MutationKind::Delete(query_builder::collection_predicate_param(
            model_type, building,
        )),
        return_type: OperationReturnType {
            type_id: model_type_id,
            type_name: model_type.name.clone(),
            type_modifier: GqlTypeModifier::List,
        },
    };

    vec![by_pk_delete, by_predicate_delete]
}

fn build_update_mutations(
    model_type_id: Id<GqlType>,
    model_type: &GqlType,
    building: &SystemContextBuilding,
) -> Vec<Mutation> {
    let data_param_type_name = model_type.update_type();
    let data_param_type_id = building
        .mutation_types
        .get_id(&data_param_type_name)
        .unwrap();

    let by_pk_update = Mutation {
        name: model_type.pk_update(),
        kind: MutationKind::Update {
            data_param: UpdateDataParameter {
                name: "data".to_string(),
                type_name: data_param_type_name.clone(),
                type_id: data_param_type_id,
            },
            predicate_param: query_builder::pk_predicate_param(model_type, building),
        },
        return_type: OperationReturnType {
            type_id: model_type_id,
            type_name: model_type.name.clone(),
            type_modifier: GqlTypeModifier::Optional,
        },
    };

    let by_predicate_update = Mutation {
        name: model_type.collection_update(),
        kind: MutationKind::Update {
            data_param: UpdateDataParameter {
                name: "data".to_string(),
                type_name: data_param_type_name,
                type_id: data_param_type_id,
            },
            predicate_param: query_builder::collection_predicate_param(model_type, building),
        },
        return_type: OperationReturnType {
            type_id: model_type_id,
            type_name: model_type.name.clone(),
            type_modifier: GqlTypeModifier::List,
        },
    };

    vec![by_pk_update, by_predicate_update]
}

fn input_creation_type_name(model_type_name: &str, container_type: Option<&str>) -> String {
    match container_type {
        Some(container_type) => format!("{}CreationInputFrom{}", model_type_name, container_type),
        None => format!("{}CreationInput", model_type_name),
    }
}

fn expanded_mutation_types(
    model_type: &GqlType,
    building: &SystemContextBuilding,
) -> Vec<(Id<GqlType>, GqlTypeKind)> {
    let mut expanded_nested_mutation_types = HashSet::new();

    let existing_type = model_type;

    if let GqlTypeKind::Composite(GqlCompositeTypeKind {
        ref fields,
        table_id,
        pk_query,
        collection_query,
        ..
    }) = existing_type.kind
    {
        let model_fields = fields;

        let reference_types = {
            let reference_type_fields = model_fields
                .clone()
                .into_iter()
                .flat_map(|field| match &field.relation {
                    GqlRelation::Pk { .. } => Some(field),
                    _ => None,
                })
                .collect();

            let existing_type_name = model_type.reference_type();
            let existing_type_id = building.mutation_types.get_id(&existing_type_name).unwrap();

            vec![(
                existing_type_id,
                GqlTypeKind::Composite(GqlCompositeTypeKind {
                    fields: reference_type_fields,
                    table_id,
                    pk_query,
                    collection_query,
                    access: Access::restrictive(),
                }),
            )]
        };

        let creation_types = expanded_creation_type(
            model_type,
            building,
            vec![],
            &mut expanded_nested_mutation_types,
        );

        let update_types = {
            let input_type_fields = compute_update_input_fields(model_fields, building);

            let existing_type_name = model_type.update_type();
            let existing_type_id = building.mutation_types.get_id(&existing_type_name).unwrap();

            vec![(
                existing_type_id,
                GqlTypeKind::Composite(GqlCompositeTypeKind {
                    fields: input_type_fields,
                    table_id,
                    pk_query,
                    collection_query,
                    access: Access::restrictive(),
                }),
            )]
        };

        vec![reference_types, creation_types, update_types]
            .into_iter()
            .flatten()
            .collect()
    } else {
        vec![]
    }
}

fn expanded_creation_type(
    model_type: &GqlType,
    building: &SystemContextBuilding,
    container_types: Vec<&str>,
    expanded_nested_mutation_types: &mut HashSet<String>,
) -> Vec<(Id<GqlType>, GqlTypeKind)> {
    if let GqlTypeKind::Composite(GqlCompositeTypeKind {
        ref fields,
        table_id,
        pk_query,
        collection_query,
        ..
    }) = model_type.kind
    {
        let model_fields = fields;

        let mut new_container_types = container_types.clone();
        new_container_types.push(&model_type.name);

        let mut creation_types: Vec<_> = model_fields
            .iter()
            .flat_map(|field| {
                let field_type = field.typ.base_type(&building.types.values);
                if let (GqlTypeKind::Composite(_), GqlFieldType::List(_)) =
                    (&field_type.kind, &field.typ)
                {
                    let existing_type_name = input_creation_type_name(
                        &field_type.name,
                        container_types.first().copied(),
                    );

                    // Protect against going into an infinite loop when cycles are present
                    if !expanded_nested_mutation_types.contains(&existing_type_name) {
                        expanded_nested_mutation_types.insert(existing_type_name);
                        expanded_creation_type(
                            field_type,
                            building,
                            new_container_types.clone(),
                            expanded_nested_mutation_types,
                        )
                    } else {
                        vec![]
                    }
                } else {
                    vec![]
                }
            })
            .collect();

        let existing_type_name =
            input_creation_type_name(model_type.name.as_str(), container_types.first().copied());
        let existing_type_id = building.mutation_types.get_id(&existing_type_name).unwrap();

        let input_type_fields =
            compute_create_input_fields(model_fields, new_container_types, building);
        creation_types.push((
            existing_type_id,
            GqlTypeKind::Composite(GqlCompositeTypeKind {
                fields: input_type_fields,
                table_id,
                pk_query,
                collection_query,
                access: Access::restrictive(),
            }),
        ));

        creation_types
    } else {
        vec![]
    }
}

fn compute_create_input_fields(
    gql_fields: &[GqlField],
    container_types: Vec<&str>,
    building: &SystemContextBuilding,
) -> Vec<GqlField> {
    gql_fields
        .iter()
        .flat_map(|field| match &field.relation {
            GqlRelation::Pk { .. } => None, // TODO: Make this decistion based on autoincrement/uuid etc of the id
            GqlRelation::Scalar { .. } => Some(GqlField {
                typ: field.typ.clone(),
                ..field.clone()
            }),
            GqlRelation::OneToMany { .. } => {
                let field_type_name = input_creation_type_name(
                    field.typ.type_name(),
                    container_types.first().copied(),
                );
                let field_type_id = building.mutation_types.get_id(&field_type_name).unwrap();
                let field_plain_type = GqlFieldType::Reference {
                    type_name: field_type_name,
                    type_id: field_type_id,
                };
                let field_type = GqlFieldType::List(Box::new(field_plain_type));

                if container_types.contains(&field.typ.type_name()) {
                    None
                } else {
                    Some(GqlField {
                        name: field.name.clone(),
                        typ: field_type,
                        relation: field.relation.clone(),
                    })
                }
            }
            GqlRelation::ManyToOne { .. } => {
                let field_type_name = field.typ.type_name().reference_type();
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

                if container_types.contains(&field.typ.type_name()) {
                    None
                } else {
                    Some(GqlField {
                        name: field.name.clone(),
                        typ: field_type,
                        relation: field.relation.clone(),
                    })
                }
            }
        })
        .collect()
}

// TODO: After many-to-many impl is complete, reexamine if we can share more code between compute_create_input_fields
// and the following function
fn compute_update_input_fields(
    gql_fields: &[GqlField],
    building: &SystemContextBuilding,
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
                let field_type_name = field.typ.type_name().reference_type();
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
                Some(GqlField {
                    name: field.name.clone(),
                    typ: field_type.optional(),
                    relation: field.relation.clone(),
                })
            }
        })
        .collect()
}
