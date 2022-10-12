use serde::{Deserialize, Serialize};

use super::column_id::ColumnId;

use super::types::DatabaseType;
use core_model::mapped_arena::SerializableSlabIndex;

// We model one-to-one (more precisely one-to-one_or_zero and one_or_zero-to-one) relations as
// a OneToMany and ManyToOne relation (respectively), so that we can share most of the logic to
// build queries etc. We use RelationCardinality to distinguish between these two cases.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum RelationCardinality {
    Optional,  // The cardinality of a "one-to-one" relation
    Unbounded, // The cardinality for a "many" relationship.
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum DatabaseRelation {
    Pk {
        column_id: ColumnId,
    },
    Scalar {
        column_id: ColumnId,
    },
    ManyToOne {
        column_id: ColumnId,
        other_type_id: SerializableSlabIndex<DatabaseType>,
        cardinality: RelationCardinality,
    },
    OneToMany {
        other_type_column_id: ColumnId,
        other_type_id: SerializableSlabIndex<DatabaseType>,
        cardinality: RelationCardinality,
    },
}

impl DatabaseRelation {
    pub fn self_column(&self) -> Option<ColumnId> {
        match self {
            DatabaseRelation::Pk { column_id }
            | DatabaseRelation::Scalar { column_id }
            | DatabaseRelation::ManyToOne { column_id, .. } => Some(*column_id),
            _ => None,
        }
    }
}
