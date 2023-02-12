//! Build mutation input types associated with deletion (<Type>DeletionInput) and
//! the create mutations (delete<Type>, and delete<Type>s)

use core_plugin_interface::core_model::{
    mapped_arena::{MappedArena, SerializableSlabIndex},
    types::{BaseOperationReturnType, OperationReturnType},
};
use postgres_model::operation::PostgresMutationKind;
use postgres_model::types::EntityType;

use super::{
    builder::Builder,
    mutation_builder::MutationBuilder,
    naming::ToPostgresMutationNames,
    query_builder,
    resolved_builder::{ResolvedCompositeType, ResolvedType},
    system_builder::SystemContextBuilding,
    type_builder::ResolvedTypeEnv,
};

pub struct DeleteMutationBuilder;

impl Builder for DeleteMutationBuilder {
    fn type_names(
        &self,
        _resolved_composite_type: &ResolvedCompositeType,
        _types: &MappedArena<ResolvedType>,
    ) -> Vec<String> {
        // delete mutations don't need any special input type (the type for the PK and the type for filtering suffice)
        vec![]
    }

    /// Expand the mutation input types as well as build the mutation
    fn build_expanded(
        &self,
        _resolved_env: &ResolvedTypeEnv,
        building: &mut SystemContextBuilding,
    ) {
        // Since there are no special input types for deletion, no expansion is needed

        for (model_type_id, model_type) in building.entity_types.iter() {
            for mutation in self.build_mutations(model_type_id, model_type, building) {
                building.mutations.add(&mutation.name.to_owned(), mutation);
            }
        }
    }
}

impl MutationBuilder for DeleteMutationBuilder {
    fn single_mutation_name(model_type: &EntityType) -> String {
        model_type.pk_delete()
    }

    fn single_mutation_kind(
        model_type_id: SerializableSlabIndex<EntityType>,
        model_type: &EntityType,
        building: &SystemContextBuilding,
    ) -> PostgresMutationKind {
        PostgresMutationKind::Delete(query_builder::pk_predicate_param(
            model_type_id,
            model_type,
            building,
        ))
    }

    fn single_mutation_modified_type(
        base_type: BaseOperationReturnType<EntityType>,
    ) -> OperationReturnType<EntityType> {
        // We return null if the specified id doesn't exist
        OperationReturnType::Optional(Box::new(OperationReturnType::Plain(base_type)))
    }

    fn multi_mutation_name(model_type: &EntityType) -> String {
        model_type.collection_delete()
    }

    fn multi_mutation_kind(
        model_type_id: SerializableSlabIndex<EntityType>,
        model_type: &EntityType,
        building: &SystemContextBuilding,
    ) -> PostgresMutationKind {
        PostgresMutationKind::Delete(query_builder::collection_predicate_param(
            model_type_id,
            model_type,
            building,
        ))
    }
}
