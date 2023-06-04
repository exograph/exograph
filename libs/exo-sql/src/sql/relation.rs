use serde::{Deserialize, Serialize};

use crate::{ColumnId, PhysicalColumnPathLink};

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct OneToMany {
    pub self_pk_column_id: ColumnId,
    pub foreign_column_id: ColumnId,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ManyToOne {
    pub self_column_id: ColumnId,
    pub foreign_pk_column_id: ColumnId,
}

impl OneToMany {
    pub fn column_path_link(&self) -> PhysicalColumnPathLink {
        PhysicalColumnPathLink::relation(self.self_pk_column_id, self.foreign_column_id)
    }
}

impl ManyToOne {
    pub fn column_path_link(&self) -> PhysicalColumnPathLink {
        PhysicalColumnPathLink::relation(self.self_column_id, self.foreign_pk_column_id)
    }
}
