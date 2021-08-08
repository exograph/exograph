//! Build update mutation types <Type>UpdateInput, update<Type>, and update<Type>s

use id_arena::Id;
use payas_model::model::access::Access;
use payas_model::model::mapped_arena::MappedArena;
use payas_model::model::naming::{ToGqlMutationNames, ToGqlTypeNames};
use payas_model::model::relation::GqlRelation;
use payas_model::model::{operation::Mutation, types::GqlType};
use payas_model::model::{GqlCompositeTypeKind, GqlField, GqlFieldType, GqlTypeKind};

use crate::builder::query_builder;

use payas_model::model::{
    operation::{MutationKind, OperationReturnType, UpdateDataParameter},
    types::GqlTypeModifier,
};

use super::builder::Builder;
use super::resolved_builder::{ResolvedCompositeType, ResolvedType};
use super::system_builder::SystemContextBuilding;

pub struct UpdateMutationBuilder;

impl Builder for UpdateMutationBuilder {
    fn type_names(
        &self,
        resolved_composite_type: &ResolvedCompositeType,
        _models: &MappedArena<ResolvedType>,
    ) -> Vec<String> {
        vec![resolved_composite_type.update_type()]
    }

    /// Expand the mutation input types as well as build the mutation
    fn build_expanded(&self, building: &mut SystemContextBuilding) {
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

                for mutation in build_update_mutations(model_type_id, model_type, building) {
                    building.mutations.add(&mutation.name.to_owned(), mutation);
                }
            }
        }
    }
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

fn expanded_mutation_types(
    model_type: &GqlType,
    building: &SystemContextBuilding,
) -> Vec<(Id<GqlType>, GqlTypeKind)> {
    let existing_type = model_type;

    if let GqlTypeKind::Composite(GqlCompositeTypeKind {
        ref fields,
        table_id,
        pk_query,
        collection_query,
        ..
    }) = existing_type.kind
    {
        let input_type_fields = compute_update_input_fields(fields, building);

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
    } else {
        vec![]
    }
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
