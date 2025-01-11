// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

//! Build the reference input type (used to refer to an entity by its pk)

use core_plugin_interface::{
    core_model::mapped_arena::{MappedArena, SerializableSlabIndex},
    core_model_builder::error::ModelBuildingError,
};
use postgres_graphql_model::types::MutationType;

use postgres_core_model::{
    relation::PostgresRelation,
    types::{EntityRepresentation, EntityType, PostgresField},
};

use crate::utils::{to_mutation_type, MutationTypeKind};

use super::{builder::Builder, naming::ToPostgresTypeNames, system_builder::SystemContextBuilding};
use postgres_core_builder::resolved_type::{ResolvedCompositeType, ResolvedType};

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
        building: &mut SystemContextBuilding,
    ) -> Result<(), ModelBuildingError> {
        for (_, entity_type) in building
            .core_subsystem
            .entity_types
            .iter()
            .filter(|(_, et)| et.representation != EntityRepresentation::Json)
        {
            for (existing_id, expanded_type) in expanded_reference_types(entity_type, building) {
                building.mutation_types[existing_id] = expanded_type;
            }
        }

        Ok(())
    }

    fn needs_mutation_type(&self, composite_type: &ResolvedCompositeType) -> bool {
        composite_type.representation != EntityRepresentation::Json
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
            PostgresRelation::Scalar { is_pk: true, .. } => Some(PostgresField {
                name: field.name.clone(),
                typ: to_mutation_type(&field.typ, MutationTypeKind::Reference, building),
                access: field.access.clone(),
                relation: field.relation.clone(),
                has_default_value: field.has_default_value,
                dynamic_default_value: None,
                readonly: field.readonly,
                type_validation: None,
            }),
            PostgresRelation::ManyToOne { is_pk: true, .. } => Some(PostgresField {
                name: field.name.clone(),
                typ: to_mutation_type(&field.typ, MutationTypeKind::Reference, building),
                access: field.access.clone(),
                relation: field.relation.clone(),
                has_default_value: field.has_default_value,
                dynamic_default_value: None,
                readonly: field.readonly,
                type_validation: None,
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
            entity_id: building
                .core_subsystem
                .entity_types
                .get_id(&entity_type.name)
                .unwrap(),
            input_access: None,
            database_access: None,
        },
    )]
}
