use serde::{Deserialize, Serialize};

use crate::{ColumnId, ColumnPathLink, Database};

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct OneToMany {
    pub self_pk_column_id: ColumnId,
    pub foreign_column_id: ColumnId,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Hash, Debug)]
pub struct ManyToOne {
    pub self_column_id: ColumnId,
    pub foreign_pk_column_id: ColumnId,
    /// A name that may be used to alias the foreign table. This is useful when
    /// multiple columns in a table refer to the same foreign table. For example,
    /// `concerts` may have a `main_venue_id` and a `alt_venue_id`.
    pub foreign_table_alias: Option<String>,
}

impl OneToMany {
    pub fn column_path_link(&self) -> ColumnPathLink {
        ColumnPathLink::relation(self.self_pk_column_id, self.foreign_column_id, None)
    }
}

impl ManyToOne {
    fn flipped(&self) -> OneToMany {
        OneToMany {
            self_pk_column_id: self.foreign_pk_column_id,
            foreign_column_id: self.self_column_id,
        }
    }

    pub fn column_path_link(&self) -> ColumnPathLink {
        ColumnPathLink::relation(
            self.self_column_id,
            self.foreign_pk_column_id,
            self.foreign_table_alias.clone(),
        )
    }
}

/// Many to one id, which is an index into the `Database.relations` vector
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ManyToOneId(pub(crate) usize);

impl ManyToOneId {
    pub fn deref(&self, database: &Database) -> ManyToOne {
        database.relations[self.0].clone()
    }
}

/// One to many id, which refers to its corresponding many to one id (`Database` keeps track of only the many to one id)
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct OneToManyId(pub(crate) ManyToOneId);

impl OneToManyId {
    pub fn deref(&self, database: &Database) -> OneToMany {
        self.0.deref(database).flipped()
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum RelationId {
    ManyToOne(ManyToOneId),
    OneToMany(OneToManyId),
}
