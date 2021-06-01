use id_arena::Id;

use super::column_id::ColumnId;

use super::types::GqlType;

#[derive(Debug, Clone)]
pub enum GqlRelation {
    Pk {
        column_id: ColumnId,
    },
    Scalar {
        column_id: ColumnId,
    },
    ManyToOne {
        column_id: ColumnId,
        other_type_id: Id<GqlType>,
        optional: bool,
    },
    OneToMany {
        other_type_column_id: ColumnId,
        other_type_id: Id<GqlType>,
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
