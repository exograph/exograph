use serde::{Deserialize, Serialize};

use crate::{ColumnId, Database, PhysicalColumnPathLink};

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
    pub fn flipped(&self) -> OneToMany {
        OneToMany {
            self_pk_column_id: self.foreign_pk_column_id,
            foreign_column_id: self.self_column_id,
        }
    }

    pub fn column_path_link(&self) -> PhysicalColumnPathLink {
        PhysicalColumnPathLink::relation(self.self_column_id, self.foreign_pk_column_id)
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ManyToOneRelationId {
    pub index: usize,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct OneToManyRelationId {
    pub underlying: ManyToOneRelationId,
}

impl ManyToOneRelationId {
    pub fn column_path_link(&self, database: &Database) -> PhysicalColumnPathLink {
        let ManyToOne {
            self_column_id,
            foreign_pk_column_id,
        } = database.get_relation(*self);

        PhysicalColumnPathLink::relation(*self_column_id, *foreign_pk_column_id)
    }
}

impl OneToManyRelationId {
    pub fn column_path_link(&self, database: &Database) -> PhysicalColumnPathLink {
        let ManyToOne {
            self_column_id,
            foreign_pk_column_id,
        } = database.get_relation(self.underlying);

        PhysicalColumnPathLink::relation(*foreign_pk_column_id, *self_column_id)
    }
}
