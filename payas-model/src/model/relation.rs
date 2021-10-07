use serde::{Deserialize, Serialize};

use super::column_id::ColumnId;

use super::mapped_arena::SerializableSlabIndex;
use super::types::GqlType;

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
        optional: bool,
    },
    OneToMany {
        other_type_column_id: ColumnId,
        other_type_id: SerializableSlabIndex<GqlType>,
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
