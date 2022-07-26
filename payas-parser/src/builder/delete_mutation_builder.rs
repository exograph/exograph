//! Build mutation input types associatd with deletion (<Type>DeletionInput) and
//! the create mutations (delete<Type>, and delete<Type>s)

use super::naming::ToGqlMutationNames;
use payas_model::model::mapped_arena::{MappedArena, SerializableSlabIndex};
use payas_model::model::types::GqlType;
use payas_model::model::{GqlCompositeType, GqlCompositeTypeKind, GqlTypeKind};

use crate::builder::query_builder;

use payas_model::model::operation::DatabaseMutationKind;

use super::mutation_builder::MutationBuilder;
use super::resolved_builder::{ResolvedCompositeType, ResolvedType};
use super::system_builder::SystemContextBuilding;
use super::Builder;

pub struct DeleteMutationBuilder;

impl Builder for DeleteMutationBuilder {
    fn type_names(
        &self,
        _resolved_composite_type: &ResolvedCompositeType,
        _models: &MappedArena<ResolvedType>,
    ) -> Vec<String> {
        // delete mutations don't need any special input type (the type for the PK and the type for filtering suffice)
        vec![]
    }

    /// Expand the mutation input types as well as build the mutation
    fn build_expanded(&self, building: &mut SystemContextBuilding) {
        // Since there are no special input types for deletion, no expansion is needed

        for (_, model_type) in building.types.iter() {
            if let GqlTypeKind::Composite(GqlCompositeType {
                kind: GqlCompositeTypeKind::Persistent { .. },
                ..
            }) = &model_type.kind
            {
                let model_type_id = building.types.get_id(model_type.name.as_str()).unwrap();

                for mutation in self.build_mutations(model_type_id, model_type, building) {
                    building.mutations.add(&mutation.name.to_owned(), mutation);
                }
            }
        }
    }
}

impl MutationBuilder for DeleteMutationBuilder {
    fn single_mutation_name(model_type: &GqlType) -> String {
        model_type.pk_delete()
    }

    fn single_mutation_kind(
        model_type_id: SerializableSlabIndex<GqlType>,
        model_type: &GqlType,
        building: &SystemContextBuilding,
    ) -> DatabaseMutationKind {
        DatabaseMutationKind::Delete(query_builder::pk_predicate_param(
            model_type_id,
            model_type,
            building,
        ))
    }

    fn multi_mutation_name(model_type: &GqlType) -> String {
        model_type.collection_delete()
    }

    fn multi_mutation_kind(
        model_type_id: SerializableSlabIndex<GqlType>,
        model_type: &GqlType,
        building: &SystemContextBuilding,
    ) -> DatabaseMutationKind {
        DatabaseMutationKind::Delete(query_builder::collection_predicate_param(
            model_type_id,
            model_type,
            building,
        ))
    }
}
