// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use crate::types::EntityType;

use core_plugin_interface::core_model::mapped_arena::SerializableSlabIndex;
use exo_sql::{ColumnId, PhysicalColumnPathLink};
use serde::{Deserialize, Serialize};

// We model one-to-one (more precisely one-to-one_or_zero and one_or_zero-to-one) relations as
// a OneToMany and ManyToOne relation (respectively), so that we can share most of the logic to
// build queries etc. We use RelationCardinality to distinguish between these two cases.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum RelationCardinality {
    Optional,  // The cardinality of a "one-to-one" relation
    Unbounded, // The cardinality for a "many" relationship.
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum PostgresRelation {
    Pk {
        column_id: ColumnId,
    },
    Scalar {
        column_id: ColumnId,
    },
    ManyToOne {
        other_type_id: SerializableSlabIndex<EntityType>,
        cardinality: RelationCardinality,
        column_id_path_link: PhysicalColumnPathLink,
    },
    // In case of Venue -> [Concert] and the enclosing type is `Venue`, we will have:
    // - other_type_id: Concert
    // - cardinality: Unbounded
    // - column_id_path_link: (self_column_id: venues.id, linked_column_id: concerts.venue_id)
    OneToMany {
        other_type_id: SerializableSlabIndex<EntityType>,
        cardinality: RelationCardinality,
        column_id_path_link: PhysicalColumnPathLink,
    },
}

impl PostgresRelation {
    pub fn self_column(&self) -> Option<ColumnId> {
        match self {
            PostgresRelation::Pk { column_id } | PostgresRelation::Scalar { column_id } => {
                Some(*column_id)
            }
            PostgresRelation::ManyToOne {
                column_id_path_link,
                ..
            } => Some(column_id_path_link.self_column_id),
            _ => None,
        }
    }

    pub fn column_path_link(&self) -> PhysicalColumnPathLink {
        match &self {
            PostgresRelation::Pk { column_id, .. } | PostgresRelation::Scalar { column_id, .. } => {
                PhysicalColumnPathLink::new(*column_id, None)
            }
            PostgresRelation::ManyToOne {
                column_id_path_link,
                ..
            } => column_id_path_link.clone(),
            PostgresRelation::OneToMany {
                column_id_path_link,
                ..
            } => column_id_path_link.clone(),
        }
    }
}
