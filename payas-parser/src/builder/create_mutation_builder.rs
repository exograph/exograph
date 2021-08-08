//! Build mutation input types associatd with creation (<Type>CreationInput) and
//! the create mutations (create<Type>, and create<Type>s)

use std::collections::HashSet;

use id_arena::Id;
use payas_model::model::access::Access;
use payas_model::model::mapped_arena::MappedArena;
use payas_model::model::naming::{ToGqlMutationNames, ToGqlTypeNames};
use payas_model::model::relation::GqlRelation;
use payas_model::model::{operation::Mutation, types::GqlType};
use payas_model::model::{GqlCompositeTypeKind, GqlField, GqlFieldType, GqlTypeKind};

use payas_model::model::{
    operation::{CreateDataParameter, MutationKind, OperationReturnType},
    types::GqlTypeModifier,
};

use super::builder::Builder;
use super::resolved_builder::{ResolvedCompositeType, ResolvedFieldType, ResolvedType};
use super::system_builder::SystemContextBuilding;

pub struct CreateMutationBuilder;

impl Builder for CreateMutationBuilder {
    fn type_names(
        &self,
        resolved_composite_type: &ResolvedCompositeType,
        models: &MappedArena<ResolvedType>,
    ) -> Vec<String> {
        let mutation_type_names = vec![resolved_composite_type.creation_type()];

        resolved_composite_type
            .fields
            .iter()
            .filter_map(|field| {
                let field_type = field.typ.deref(models);
                // Create a nested input data type only if it refers to a many side
                // So for Venue <-> [Concert] case, create only ConcertCreationInputFromVenue
                if let ResolvedFieldType::List(_) = field.typ {
                    if let ResolvedType::Composite(ResolvedCompositeType { name, .. }) = field_type
                    {
                        Some(input_creation_type_name(
                            name,
                            Some(&resolved_composite_type.name),
                        ))
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .chain(mutation_type_names.into_iter())
            .collect()
    }

    fn build_expanded(&self, building: &mut SystemContextBuilding) {
        let mut expanded_nested_mutation_types = HashSet::new();

        for (_, model_type) in building.types.iter() {
            if let GqlTypeKind::Composite { .. } = &model_type.kind {
                for (existing_id, expanded_kind) in expanded_creation_type(
                    model_type,
                    building,
                    vec![],
                    &mut expanded_nested_mutation_types,
                ) {
                    building.mutation_types[existing_id].kind = expanded_kind;
                }
            }
        }
        for (_, model_type) in building.types.iter() {
            if let GqlTypeKind::Composite { .. } = &model_type.kind {
                let model_type_id = building.types.get_id(model_type.name.as_str()).unwrap();

                for mutation in build_create_mutation(model_type_id, model_type, building) {
                    building.mutations.add(&mutation.name.to_owned(), mutation);
                }
            }
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

fn input_creation_type_name(model_type_name: &str, container_type: Option<&str>) -> String {
    match container_type {
        Some(container_type) => {
            format!("{}From{}", model_type_name.creation_type(), container_type)
        }
        None => model_type_name.creation_type(),
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
