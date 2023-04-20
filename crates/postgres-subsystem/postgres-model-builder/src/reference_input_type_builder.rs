// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

//! Build the reference input type (used to refer to an entity by its pk)

use core_plugin_interface::core_model::mapped_arena::{MappedArena, SerializableSlabIndex};
use postgres_model::{
    relation::PostgresRelation,
    types::{EntityType, MutationType, PostgresField},
};

use crate::utils::to_mutation_type;

use super::{
    builder::Builder,
    naming::ToPostgresTypeNames,
    resolved_builder::{ResolvedCompositeType, ResolvedType},
    system_builder::SystemContextBuilding,
    type_builder::ResolvedTypeEnv,
};

pub struct ReferenceInputTypeBuilder;

impl Builder for ReferenceInputTypeBuilder {
    fn type_names(
        &self,
        resolved_composite_type: &ResolvedCompositeType,
        _types: &MappedArena<ResolvedType>,
    ) -> Vec<String> {
        vec![resolved_composite_type.reference_type()]
    }

    /// Expand the mutation input types as well as build the mutation
    fn build_expanded(
        &self,
        _resolved_env: &ResolvedTypeEnv,
        building: &mut SystemContextBuilding,
    ) {
        for (_, entity_type) in building.entity_types.iter() {
            for (existing_id, expanded_type) in expanded_reference_types(entity_type, building) {
                building.mutation_types[existing_id] = expanded_type;
            }
        }
    }
}

fn expanded_reference_types(
    entity_type: &EntityType,
    building: &SystemContextBuilding,
) -> Vec<(SerializableSlabIndex<MutationType>, MutationType)> {
    let reference_type_fields = entity_type
        .fields
        .iter()
        .flat_map(|field| match &field.relation {
            PostgresRelation::Pk { .. } => Some(PostgresField {
                name: field.name.clone(),
                typ: to_mutation_type(&field.typ),
                relation: field.relation.clone(),
                has_default_value: field.has_default_value,
            }),
            _ => None,
        })
        .collect();

    let existing_type_name = entity_type.reference_type();
    let existing_type_id = building.mutation_types.get_id(&existing_type_name).unwrap();

    vec![(
        existing_type_id,
        MutationType {
            name: existing_type_name,
            fields: reference_type_fields,
            entity_type: building.entity_types.get_id(&entity_type.name).unwrap(),
        },
    )]
}
