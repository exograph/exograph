// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

//! Build mutation input types associated with deletion (`<Type>DeletionInput`) and
//! the create mutations (`delete<Type>`, and `delete<Type>s`)

use core_plugin_interface::core_model::{
    mapped_arena::MappedArena,
    types::{BaseOperationReturnType, OperationReturnType},
};
use postgres_model::mutation::PostgresMutationParameters;
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

        for (entity_type_id, entity_type) in building.entity_types.iter() {
            for mutation in self.build_mutations(entity_type_id, entity_type, building) {
                building.mutations.add(&mutation.name.to_owned(), mutation);
            }
        }
    }
}

impl MutationBuilder for DeleteMutationBuilder {
    fn single_mutation_name(entity_type: &EntityType) -> String {
        entity_type.pk_delete()
    }

    fn single_mutation_parameters(
        entity_type: &EntityType,
        building: &SystemContextBuilding,
    ) -> PostgresMutationParameters {
        PostgresMutationParameters::Delete(query_builder::pk_predicate_param(
            entity_type,
            &building.predicate_types,
            &building.database,
        ))
    }

    fn single_mutation_modified_type(
        base_type: BaseOperationReturnType<EntityType>,
    ) -> OperationReturnType<EntityType> {
        // We return null if the specified id doesn't exist
        OperationReturnType::Optional(Box::new(OperationReturnType::Plain(base_type)))
    }

    fn multi_mutation_name(entity_type: &EntityType) -> String {
        entity_type.collection_delete()
    }

    fn multi_mutation_parameters(
        entity_type: &EntityType,
        building: &SystemContextBuilding,
    ) -> PostgresMutationParameters {
        PostgresMutationParameters::Delete(query_builder::collection_predicate_param(
            entity_type,
            &building.predicate_types,
        ))
    }
}
