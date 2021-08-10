//! Build update mutation types <Type>UpdateInput, update<Type>, and update<Type>s

use std::collections::HashSet;

use id_arena::Id;
use payas_model::model::mapped_arena::MappedArena;
use payas_model::model::naming::{ToGqlMutationNames, ToGqlTypeNames};
use payas_model::model::types::GqlType;
use payas_model::model::GqlTypeKind;

use crate::builder::query_builder;

use payas_model::model::operation::{MutationKind, UpdateDataParameter};

use super::mutation_builder::CreateUpdateBuilder;
use super::resolved_builder::{ResolvedCompositeType, ResolvedType};
use super::system_builder::SystemContextBuilding;
use super::Builder;

pub struct UpdateMutationBuilder;

impl Builder for UpdateMutationBuilder {
    fn type_names(
        &self,
        resolved_composite_type: &ResolvedCompositeType,
        models: &MappedArena<ResolvedType>,
    ) -> Vec<String> {
        let mutation_type_names = vec![resolved_composite_type.update_type()];

        self.field_type_names(resolved_composite_type, models)
            .into_iter()
            .chain(mutation_type_names.into_iter())
            .collect()
    }

    /// Expand the mutation input types as well as build the mutation
    fn build_expanded(&self, building: &mut SystemContextBuilding) {
        let mut expanded_nested_mutation_types = HashSet::new();

        for (_, model_type) in building.types.iter() {
            if let GqlTypeKind::Composite { .. } = &model_type.kind {
                for (existing_id, expanded_kind) in self.expanded_type(
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

                for mutation in self.build_mutations(model_type_id, model_type, building) {
                    building.mutations.add(&mutation.name.to_owned(), mutation);
                }
            }
        }
    }
}

impl CreateUpdateBuilder for UpdateMutationBuilder {
    fn mark_fields_optional() -> bool {
        true
    }

    fn base_input_type_name(model_type_name: &str) -> String {
        model_type_name.update_type()
    }

    fn single_mutation_name(model_type: &GqlType) -> String {
        model_type.pk_update()
    }

    fn single_mutation_kind(
        model_type: &GqlType,
        param_type_name: &str,
        param_type_id: Id<GqlType>,
        building: &SystemContextBuilding,
    ) -> MutationKind {
        MutationKind::Update {
            data_param: UpdateDataParameter {
                name: "data".to_string(),
                type_name: param_type_name.to_string(),
                type_id: param_type_id,
            },
            predicate_param: query_builder::pk_predicate_param(model_type, building),
        }
    }

    fn multi_mutation_name(model_type: &GqlType) -> String {
        model_type.collection_update()
    }

    fn multi_mutation_kind(
        model_type: &GqlType,
        param_type_name: &str,
        param_type_id: Id<GqlType>,
        building: &SystemContextBuilding,
    ) -> MutationKind {
        MutationKind::Update {
            data_param: UpdateDataParameter {
                name: "data".to_string(),
                type_name: param_type_name.to_string(),
                type_id: param_type_id,
            },
            predicate_param: query_builder::collection_predicate_param(model_type, building),
        }
    }
}
