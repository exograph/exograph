use serde::{Deserialize, Serialize};

use super::column_id::ColumnId;

use super::mapped_arena::SerializableSlabIndex;
use super::types::GqlType;

// We model one-to-one (more precisely one-to-one_or_zero and one_or_zero-to-one) relations as
// a OneToMany and ManyToOne relation (respectively), so that we can share most of the logic to
// build queries etc. We use RelationCardinality to distinguish between these two cases.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum RelationCardinality {
    Optional,  // The cardinality of a "one-to-one" relation
    Unbounded, // The cardinality for a "many" relationship.
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum GqlRelation {
    NonPersistent,
    Pk {
        column_id: ColumnId,
    },
    Scalar {
        column_id: ColumnId,
    },
    ManyToOne {
        column_id: ColumnId,
        other_type_id: SerializableSlabIndex<GqlType>,
        cardinality: RelationCardinality,
    },
    OneToMany {
        other_type_column_id: ColumnId,
        other_type_id: SerializableSlabIndex<GqlType>,
        cardinality: RelationCardinality,
    },
}

impl GqlRelation {
    pub fn self_column(&self) -> Option<ColumnId> {
        match self {
            GqlRelation::Pk { column_id }
            | GqlRelation::Scalar { column_id }
            | GqlRelation::ManyToOne { column_id, .. } => Some(column_id.clone()),
            _ => None,
        }
    }
}
