use serde::{Deserialize, Serialize};

use crate::ColumnId;

#[derive(Serialize, Deserialize, PartialEq, Eq, Hash, Debug)]
pub struct ManyToOne {
    pub self_pk_column_id: ColumnId,
    pub foreign_column_id: ColumnId,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub struct OneToMany {
    pub self_column_id: ColumnId,
    pub foreign_column_id: ColumnId,
}
