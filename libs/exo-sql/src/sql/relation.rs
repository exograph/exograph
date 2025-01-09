use serde::{Deserialize, Serialize};

use crate::{ColumnId, ColumnPathLink, Database, TableId};

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct RelationColumnPair {
    pub self_column_id: ColumnId,
    pub foreign_column_id: ColumnId,
}

impl RelationColumnPair {
    pub fn flipped(&self) -> RelationColumnPair {
        RelationColumnPair {
            self_column_id: self.foreign_column_id,
            foreign_column_id: self.self_column_id,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Hash, Debug)]
pub struct OneToMany {
    pub column_pairs: Vec<RelationColumnPair>,
}

impl OneToMany {
    pub fn new(column_pairs: Vec<RelationColumnPair>) -> Self {
        assert!(!column_pairs.is_empty());
        Self { column_pairs }
    }

    pub fn self_table_id(&self) -> TableId {
        self.column_pairs[0].self_column_id.table_id
    }

    pub fn linked_table_id(&self) -> TableId {
        self.column_pairs[0].foreign_column_id.table_id
    }

    pub fn column_path_link(&self) -> ColumnPathLink {
        ColumnPathLink::relation(self.column_pairs.clone(), None)
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Hash, Debug)]
pub struct ManyToOne {
    pub column_pairs: Vec<RelationColumnPair>,
    /// A name that may be used to alias the foreign table. This is useful when
    /// multiple columns in a table refer to the same foreign table. For example,
    /// `concerts` may have a `main_venue_id` and a `alt_venue_id`.
    pub foreign_table_alias: Option<String>,
}

impl ManyToOne {
    pub fn new(column_pairs: Vec<RelationColumnPair>, foreign_table_alias: Option<String>) -> Self {
        assert!(!column_pairs.is_empty());
        Self {
            column_pairs,
            foreign_table_alias,
        }
    }

    fn flipped(&self) -> OneToMany {
        OneToMany::new(
            self.column_pairs
                .iter()
                .map(|pair| pair.flipped())
                .collect(),
        )
    }

    pub fn column_path_link(&self) -> ColumnPathLink {
        ColumnPathLink::relation(self.column_pairs.clone(), self.foreign_table_alias.clone())
    }

    pub fn self_table_id(&self) -> TableId {
        self.column_pairs[0].self_column_id.table_id
    }

    pub fn linked_table_id(&self) -> TableId {
        self.column_pairs[0].foreign_column_id.table_id
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
