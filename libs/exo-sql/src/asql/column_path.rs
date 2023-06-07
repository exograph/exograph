// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::cmp::Ordering;

use serde::{Deserialize, Serialize};

use crate::{
    sql::{predicate::ParamEquality, SQLParamContainer},
    ColumnId, Database, TableId,
};

/// A link in `ColumnPath` to a column starting at a root table and ending at a leaf column. This
/// allows us to represent a column path that goes through multiple tables and help the query
/// planner to determine which tables to join or perform subselects. For example, to represent the
/// path starting at the concert table and ending at the artist.name column, we would have:
/// ```text
/// [
///    { self_column: ("concert", "id"), linked_column: ("concert_artist", "concert_id") },
///    { self_column: ("concert_artist", "artist_id"), linked_column: ("artist", "id") },
///    { self_column: ("artist", "name"), linked_column: None },
/// ]
/// ```
#[derive(Debug, PartialEq, Clone)]
pub enum ColumnPath {
    Physical(PhysicalColumnPath),
    Param(SQLParamContainer),
    Null,
}

impl ParamEquality for ColumnPath {
    fn param_eq(&self, other: &Self) -> Option<bool> {
        match (self, other) {
            (Self::Param(v1), Self::Param(v2)) => Some(v1 == v2),
            _ => None,
        }
    }
}

impl PartialOrd for ColumnPathLink {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ColumnPathLink {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (ColumnPathLink::Relation(relation), ColumnPathLink::Relation(other)) => {
                relation.cmp(other)
            }
            (ColumnPathLink::Leaf(column_id), ColumnPathLink::Leaf(other)) => column_id.cmp(other),
            (ColumnPathLink::Relation(_), ColumnPathLink::Leaf(_))
            | (ColumnPathLink::Leaf(_), ColumnPathLink::Relation(_)) => {
                panic!("Cannot compare a relation to a leaf")
            }
        }
    }
}

/// A link in [`ColumnPath`] to connect two tables.
/// Contains two columns that link one table to another, which may be used to form a join between two tables
#[derive(Serialize, Deserialize, Debug, Clone, Hash, PartialEq, Eq)]
pub enum ColumnPathLink {
    Relation(RelationLink),
    Leaf(ColumnId),
}

impl ColumnPathLink {
    pub fn relation(self_column_id: ColumnId, linked_column_id: ColumnId) -> Self {
        Self::Relation(RelationLink {
            self_column_id,
            foreign_column_id: linked_column_id,
        })
    }

    pub fn self_column_id(&self) -> ColumnId {
        match self {
            ColumnPathLink::Relation(relation) => relation.self_column_id,
            ColumnPathLink::Leaf(column_id) => *column_id,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Hash, PartialEq, Eq)]
pub struct RelationLink {
    /// The column in the current table that is linked to the next table.
    pub self_column_id: ColumnId,
    /// The column in the next table that is linked to the current table. None implies that this is a terminal column (such as artist.name).
    pub foreign_column_id: ColumnId,
}

impl PartialOrd for RelationLink {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for RelationLink {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.self_column_id, self.foreign_column_id)
            .cmp(&(other.self_column_id, other.foreign_column_id))
    }
}

impl ColumnPathLink {
    /// Determines if this link is a one-to-many link.
    ///
    /// If the self column is a primary key and the linked column links to a table, then this is a
    /// one-to-many link. For example, when referring from a venue to concerts, the `venue.id` would
    /// be the self column and `concert.venue_id` would be the linked column.
    pub fn is_one_to_many(&self, database: &Database) -> bool {
        match self {
            ColumnPathLink::Relation(RelationLink { self_column_id, .. }) => {
                self_column_id.get_column(database).is_pk
            }
            ColumnPathLink::Leaf(_) => false,
        }
    }
}
/// A list of path from that represent a relation between two tables
/// For example to reach concert -> concert_artist -> artist -> name,
/// the path would be [(concert.id, concert_artist.concert_id), (concert_artists.artists_id, artist.id), (artist.name, None)]
/// This information could be used to form a join between multiple tables
/// Invariant:
/// - The path is non-empty
/// - For any two consecutive links: `first_link.linked_column_id.table_id == second_link.self_column_id.table_id`
///
/// Once fully constructed: (TODO: Make Builder a separate type so we can support this invariant properly)
/// - The last link in the path is a leaf column (once fully constructed)
#[derive(Serialize, Deserialize, Debug, Clone, Hash, PartialEq, Eq)]
pub struct PhysicalColumnPath(Vec<ColumnPathLink>);

impl PhysicalColumnPath {
    /// Initialize with a head link
    /// Typically used along with `push` to build a path
    pub fn init(head: ColumnPathLink) -> Self {
        Self(vec![head])
    }

    /// Construct a simple leaf column path
    pub fn leaf(column_id: ColumnId) -> Self {
        Self::init(ColumnPathLink::Leaf(column_id))
    }

    pub fn split_head(&self) -> (ColumnPathLink, Option<PhysicalColumnPath>) {
        // We can assume that the path is non-empty due to the invariants
        let mut path = self.0.clone();
        let head = path.remove(0);

        (
            head,
            if path.is_empty() {
                None
            } else {
                Some(PhysicalColumnPath(path))
            },
        )
    }

    pub fn leaf_column(&self) -> ColumnId {
        match self.0.last().unwrap() {
            ColumnPathLink::Relation(_) => unreachable!("Invariant: last link must be a leaf"),
            ColumnPathLink::Leaf(column_id) => *column_id,
        }
    }

    pub fn has_one_to_many(&self, database: &Database) -> bool {
        self.0.iter().any(|link| link.is_one_to_many(database))
    }

    pub fn lead_table_id(&self) -> TableId {
        self.0[0].self_column_id().table_id
    }

    pub fn push(mut self, link: ColumnPathLink) -> Self {
        // Assert that the the last link in the path points to the same table as the new link's self table
        // This checks for the last two invariants (see above):
        // the last link must be a relation and its table must be the same as the new link's self table
        assert!(
            matches!(
                self.0.last().unwrap(),
                ColumnPathLink::Relation(RelationLink {
                    foreign_column_id,
                    ..
                }) if foreign_column_id.table_id == link.self_column_id().table_id
            ),
            "Expected link to point to next table"
        );

        self.0.push(link);

        self
    }

    pub fn from_columns(columns: Vec<ColumnId>, database: &Database) -> Self {
        assert!(
            !columns.is_empty(),
            "Cannot create a column path from an empty list of columns"
        );

        let mut new_path = None::<PhysicalColumnPath>;

        for (index, column_id) in columns.iter().enumerate() {
            let next_column_id = columns.get(index + 1);

            let link = match next_column_id {
                Some(next_column_id) => {
                    let next_table = next_column_id.table_id;

                    if next_table == column_id.table_id {
                        column_id
                            .get_otm_relation(database)
                            .unwrap()
                            .deref(database)
                            .column_path_link()
                    } else {
                        column_id
                            .get_mto_relation(database)
                            .unwrap()
                            .deref(database)
                            .column_path_link()
                    }
                }
                None => ColumnPathLink::Leaf(*column_id),
            };

            new_path = match new_path {
                Some(new_path) => Some(new_path.push(link)),
                None => Some(PhysicalColumnPath::init(link)),
            };
        }

        // Due to the assertion that the list of columns is non-empty, we can unwrap here
        new_path.unwrap()
    }
}
