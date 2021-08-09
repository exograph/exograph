//! Build mutation input types associatd with deletion (<Type>DeletionInput) and
//! the create mutations (delete<Type>, and delete<Type>s)

use id_arena::Id;

use payas_model::model::mapped_arena::MappedArena;
use payas_model::model::naming::ToGqlMutationNames;
use payas_model::model::GqlTypeKind;
use payas_model::model::{operation::Mutation, types::GqlType};

use crate::builder::query_builder;

use payas_model::model::{
    operation::{MutationKind, OperationReturnType},
    types::GqlTypeModifier,
};

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
            if let GqlTypeKind::Composite { .. } = &model_type.kind {
                let model_type_id = building.types.get_id(model_type.name.as_str()).unwrap();

                for mutation in build_delete_mutations(model_type_id, model_type, building) {
                    building.mutations.add(&mutation.name.to_owned(), mutation);
                }
            }
        }
    }
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
