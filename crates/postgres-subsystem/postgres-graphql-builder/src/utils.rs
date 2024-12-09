// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use core_plugin_interface::core_model::types::FieldType;
use postgres_core_model::types::{EntityRepresentation, EntityType, PostgresFieldType, TypeIndex};
use postgres_graphql_model::types::MutationType;

use crate::{naming::ToPostgresTypeNames, system_builder::SystemContextBuilding};

pub(super) enum MutationTypeKind {
    Create,
    Update,
    Reference,
}

pub(super) fn to_mutation_type(
    field_type: &FieldType<PostgresFieldType<EntityType>>,
    kind: MutationTypeKind,
    building: &SystemContextBuilding,
) -> FieldType<PostgresFieldType<MutationType>> {
    match field_type {
        FieldType::Plain(PostgresFieldType { type_id, type_name }) => match type_id {
            TypeIndex::Primitive(index) => FieldType::Plain(PostgresFieldType {
                type_id: TypeIndex::Primitive(*index),
                type_name: type_name.clone(),
            }),
            TypeIndex::Composite(index) => {
                let entity_type = &building.core_subsystem.entity_types[*index];

                if entity_type.representation == EntityRepresentation::Normal {
                    panic!("Composite field in mutation: {:?}", type_name);
                }
                let entity_type_name = &entity_type.name;

                let mutation_type_name = match kind {
                    MutationTypeKind::Create => entity_type_name.creation_type(),
                    MutationTypeKind::Update => entity_type_name.update_type(),
                    MutationTypeKind::Reference => entity_type_name.reference_type(),
                };
                let mutation_type_index =
                    building.mutation_types.get_id(&mutation_type_name).unwrap();

                FieldType::Plain(PostgresFieldType {
                    type_id: TypeIndex::Composite(mutation_type_index),
                    type_name: mutation_type_name.clone(),
                })
            }
        },
        FieldType::Optional(ft) => {
            FieldType::Optional(Box::new(to_mutation_type(ft, kind, building)))
        }
        FieldType::List(ft) => FieldType::List(Box::new(to_mutation_type(ft, kind, building))),
    }
}
