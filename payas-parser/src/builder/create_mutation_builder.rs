//! Build mutation input types associatd with creation (<Type>CreationInput) and
//! the create mutations (create<Type>, and create<Type>s)

use std::collections::HashSet;

use payas_model::model::mapped_arena::MappedArena;
use payas_model::model::naming::{ToGqlMutationNames, ToGqlTypeNames};
use payas_model::model::types::GqlType;
use payas_model::model::GqlTypeKind;

use payas_model::model::operation::{CreateDataParameter, MutationKind};

use super::mutation_builder::{DataParamBuilder, MutationBuilder};
use super::resolved_builder::{ResolvedCompositeType, ResolvedType};
use super::system_builder::SystemContextBuilding;
use super::Builder;

pub struct CreateMutationBuilder;

impl Builder for CreateMutationBuilder {
    fn type_names(
        &self,
        resolved_composite_type: &ResolvedCompositeType,
        models: &MappedArena<ResolvedType>,
    ) -> Vec<String> {
        let mutation_type_names = vec![resolved_composite_type.creation_type()];

        self.field_type_names(resolved_composite_type, models)
            .into_iter()
            .chain(mutation_type_names.into_iter())
            .collect()
    }

    fn build_expanded(&self, building: &mut SystemContextBuilding) {
        let mut expanded_nested_mutation_types = HashSet::new();

        for (_, model_type) in building.types.iter() {
            if let GqlTypeKind::Composite { .. } = &model_type.kind {
                for (existing_id, expanded_kind) in self.expanded_data_type(
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

impl MutationBuilder for CreateMutationBuilder {
    fn single_mutation_name(model_type: &GqlType) -> String {
        model_type.pk_create()
    }

    fn single_mutation_kind(
        model_type: &GqlType,
        building: &SystemContextBuilding,
    ) -> MutationKind {
        MutationKind::Create(Self::data_param(model_type, building, false))
    }

    fn multi_mutation_name(model_type: &GqlType) -> String {
        model_type.collection_create()
    }

    fn multi_mutation_kind(model_type: &GqlType, building: &SystemContextBuilding) -> MutationKind {
        MutationKind::Create(Self::data_param(model_type, building, true))
    }
}

impl DataParamBuilder<CreateDataParameter> for CreateMutationBuilder {
    fn mark_fields_optional() -> bool {
        false
    }

    fn base_data_type_name(model_type_name: &str) -> String {
        model_type_name.creation_type()
    }

    fn data_param(
        model_type: &GqlType,
        building: &SystemContextBuilding,
        array: bool,
    ) -> CreateDataParameter {
        let data_param_type_name = Self::base_data_type_name(&model_type.name);
        let data_param_type_id = building
            .mutation_types
            .get_id(&data_param_type_name)
            .unwrap();

        CreateDataParameter {
            name: "data".to_string(),
            type_name: data_param_type_name.to_string(),
            type_id: data_param_type_id,
            array_input: array,
        }
    }
}
