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

use core_model::{
    access::AccessPredicateExpression,
    mapped_arena::MappedArena,
    types::{BaseOperationReturnType, OperationReturnType},
};
use core_model_builder::error::ModelBuildingError;
use postgres_core_model::types::{EntityRepresentation, EntityType};
use postgres_graphql_model::mutation::PostgresMutationParameters;

use super::{
    builder::Builder, mutation_builder::MutationBuilder, naming::ToPostgresMutationNames,
    query_builder, system_builder::SystemContextBuilding,
};

use postgres_core_builder::resolved_type::ResolvedCompositeType;
use postgres_core_builder::resolved_type::ResolvedType;

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
        building: &mut SystemContextBuilding,
    ) -> Result<(), ModelBuildingError> {
        // Since there are no special input types for deletion, no expansion is needed
        for (entity_type_id, entity_type) in building.core_subsystem.entity_types.iter() {
            if let AccessPredicateExpression::BooleanLiteral(false) = building
                .core_subsystem
                .database_access_expressions
                .lock()
                .unwrap()[entity_type.access.delete]
            {
                continue;
            }
            for mutation in self.build_mutations(entity_type_id, entity_type, building) {
                building.mutations.add(&mutation.name.to_owned(), mutation);
            }
        }

        Ok(())
    }

    fn needs_mutation_type(&self, composite_type: &ResolvedCompositeType) -> bool {
        // Skip mutation types for Json
        composite_type.representation != EntityRepresentation::Json
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
        PostgresMutationParameters::Delete(query_builder::pk_predicate_params(
            entity_type,
            &building.core_subsystem.predicate_types,
            &building.core_subsystem.database,
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
        PostgresMutationParameters::Delete(vec![query_builder::collection_predicate_param(
            entity_type,
            &building.core_subsystem.predicate_types,
        )])
    }

    fn single_mutation_doc_comments(entity_type: &EntityType) -> Option<String> {
        Some(format!(
            "Delete the {} with the provided primary key.",
            entity_type.name
        ))
    }

    fn multi_mutation_doc_comments(entity_type: &EntityType) -> Option<String> {
        Some(format!(
            "Delete multiple {}s matching the provided `where` filter.",
            entity_type.name
        ))
    }
}
