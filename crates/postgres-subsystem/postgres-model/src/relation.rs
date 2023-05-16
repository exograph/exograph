// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use crate::{column_path::ColumnIdPathLink, types::EntityType};

use super::column_id::ColumnId;
use core_plugin_interface::core_model::mapped_arena::SerializableSlabIndex;
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
        column_id: ColumnId,
        other_type_id: SerializableSlabIndex<EntityType>,
        cardinality: RelationCardinality,
        column_id_path_link: ColumnIdPathLink,
    },
    OneToMany {
        other_type_column_id: ColumnId,
        other_type_id: SerializableSlabIndex<EntityType>,
        cardinality: RelationCardinality,
        column_id_path_link: ColumnIdPathLink,
    },
}

impl PostgresRelation {
    pub fn self_column(&self) -> Option<ColumnId> {
        match self {
            PostgresRelation::Pk { column_id }
            | PostgresRelation::Scalar { column_id }
            | PostgresRelation::ManyToOne { column_id, .. } => Some(*column_id),
            _ => None,
        }
    }
}
