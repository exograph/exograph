// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use core_plugin_interface::core_model::mapped_arena::MappedArena;
use postgres_model::{
    column_path::ColumnIdPathLink,
    relation::PostgresRelation,
    types::{EntityType, PostgresField},
};

pub fn column_path_link(
    container_type: &EntityType,
    field: &PostgresField<EntityType>,
    entity_types: &MappedArena<EntityType>,
) -> ColumnIdPathLink {
    match &field.relation {
        PostgresRelation::Pk { column_id, .. } | PostgresRelation::Scalar { column_id, .. } => {
            ColumnIdPathLink::new(*column_id, None)
        }
        PostgresRelation::ManyToOne {
            column_id,
            other_type_id,
            ..
        } => {
            let other_type = &entity_types[*other_type_id];
            ColumnIdPathLink::new(*column_id, other_type.pk_column_id())
        }
        PostgresRelation::OneToMany {
            other_type_column_id,
            ..
        } => {
            let parent_column_id = container_type.pk_column_id().unwrap();
            ColumnIdPathLink::new(parent_column_id, Some(*other_type_column_id))
        }
    }
}
