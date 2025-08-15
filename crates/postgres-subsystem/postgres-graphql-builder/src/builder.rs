// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

//! Transforms an AstSystem into a GraphQL system

use core_model::mapped_arena::{MappedArena, SerializableSlabIndex};
use core_model_builder::error::ModelBuildingError;
use postgres_graphql_model::types::MutationType;

use postgres_core_builder::shallow::Shallow;

use super::system_builder::SystemContextBuilding;

use postgres_core_builder::resolved_type::ResolvedCompositeType;
use postgres_core_builder::resolved_type::ResolvedType;

// TODO: Ensure it works for all builders (this one makes the assumption that it is building only input types)
// TODO: Abstract out build_expanded (currently loops in it are repeated in each implementation)

/// Trait for all builders to abstract out the implementation of shallow and expanded building
pub trait Builder {
    /// Names of types produced by this builder.
    /// Shallow building use these type names (since not much else is needed)
    fn type_names(
        &self,
        resolved_composite_type: &ResolvedCompositeType,
        resolved_types: &MappedArena<ResolvedType>,
    ) -> Vec<String>;

    fn create_shallow_type(
        &self,
        resolved_composite_type: &ResolvedCompositeType,
        resolved_types: &MappedArena<ResolvedType>,
        building: &mut SystemContextBuilding,
    ) {
        for type_name in self
            .type_names(resolved_composite_type, resolved_types)
            .iter()
        {
            building.mutation_types.add(
                type_name,
                MutationType {
                    name: type_name.to_string(),
                    fields: vec![],
                    entity_id: SerializableSlabIndex::shallow(),
                    database_access: None,
                    doc_comments: None,
                },
            );
        }
    }

    fn build_shallow(
        &self,
        resolved_types: &MappedArena<ResolvedType>,
        building: &mut SystemContextBuilding,
    ) {
        for (_, resolved_type) in resolved_types.iter() {
            if let ResolvedType::Composite(composite_type) = &resolved_type
                && self.needs_mutation_type(composite_type)
            {
                self.create_shallow_type(composite_type, resolved_types, building);
            }
        }
    }

    fn needs_mutation_type(&self, composite_type: &ResolvedCompositeType) -> bool;

    fn build_expanded(
        &self,
        building: &mut SystemContextBuilding,
    ) -> Result<(), ModelBuildingError>;
}
