// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::{cmp::Ordering, marker::PhantomData};

use serde::{Deserialize, Serialize};

use crate::{
    AbstractPredicate, ColumnId, Database, TableId,
    sql::{SQLParamContainer, predicate::ParamEquality, relation::RelationColumnPair},
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
    Predicate(Box<AbstractPredicate>), // TODO: Generalize this to be any expression
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
    pub fn relation(
        column_pairs: Vec<RelationColumnPair>,
        linked_table_alias: Option<String>,
    ) -> Self {
        assert!(!column_pairs.is_empty(), "Column pairs must not be empty");

        assert!(
            column_pairs
                .iter()
                .all(|RelationColumnPair { self_column_id, .. }| {
                    column_pairs.iter().all(|column_pair| {
                        column_pair.self_column_id.table_id == self_column_id.table_id
                    })
                }),
            "All self columns in the column pairs must refer to the same table"
        );

        assert!(
            column_pairs.iter().all(
                |RelationColumnPair {
                     foreign_column_id, ..
                 }| {
                    column_pairs.iter().all(|column_pair| {
                        column_pair.foreign_column_id.table_id == foreign_column_id.table_id
                    })
                }
            ),
            "All foreign columns in the column pairs must refer to the same table"
        );

        Self::Relation(RelationLink::new(column_pairs, linked_table_alias))
    }

    pub fn self_column_ids(&self) -> Vec<ColumnId> {
        match self {
            ColumnPathLink::Relation(relation) => relation
                .column_pairs
                .iter()
                .map(|pair| pair.self_column_id)
                .collect(),
            ColumnPathLink::Leaf(column_id) => vec![*column_id],
        }
    }

    pub fn self_table_id(&self) -> TableId {
        match self {
            ColumnPathLink::Relation(relation) => relation.self_table_id,
            ColumnPathLink::Leaf(column_id) => column_id.table_id,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Hash, PartialEq, Eq)]
pub struct RelationLink {
    pub column_pairs: Vec<RelationColumnPair>,

    pub self_table_id: TableId,
    pub linked_table_id: TableId,

    /// Alias that could be used when joining the table, etc. Useful when multiple columns in the self table refers to the same linked column
    /// For example, if "concerts" has "main_venue_id" and "alternative_venue_id" (both link to the venues.id column), we can set linked_table_alias
    /// to "main_venue_id_table" and "alternative_venue_id_table" respectively. Then we can join the venues table twice with different aliases.
    /// The alias name should not matter as long as it is unique within the self table
    pub linked_table_alias: Option<String>,

    _phantom: PhantomData<()>,
}

impl RelationLink {
    pub fn new(column_pairs: Vec<RelationColumnPair>, linked_table_alias: Option<String>) -> Self {
        let (self_table_id, linked_table_id) = match &column_pairs[..] {
            [first, ..] => {
                assert!(
                    column_pairs.iter().all(|pair| {
                        pair.self_column_id.table_id == first.self_column_id.table_id
                            && pair.foreign_column_id.table_id == first.foreign_column_id.table_id
                    }),
                    "All self columns in the column pairs must refer to the same table"
                );

                (
                    first.self_column_id.table_id,
                    first.foreign_column_id.table_id,
                )
            }
            _ => unreachable!("Column pairs must not be empty"),
        };

        Self {
            column_pairs,
            self_table_id,
            linked_table_id,
            linked_table_alias,
            _phantom: PhantomData,
        }
    }

    pub fn with_alias(mut self, alias: String) -> Self {
        self.linked_table_alias = Some(alias);
        self
    }
}

impl PartialOrd for RelationLink {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for RelationLink {
    fn cmp(&self, other: &Self) -> Ordering {
        (&self.column_pairs, &self.linked_table_alias)
            .cmp(&(&other.column_pairs, &other.linked_table_alias))
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
            ColumnPathLink::Relation(RelationLink { column_pairs, .. }) => column_pairs
                .iter()
                .any(|pair| pair.self_column_id.get_column(database).is_pk),
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
        // The ColumnPathLink construction ensures that all links point to the same table, so we can use the first (or any) to get the table id
        self.0[0].self_column_ids()[0].table_id
    }

    pub fn push(mut self, link: ColumnPathLink) -> Self {
        // Assert that the the last link in the path points to the same table as the new link's self table
        // This checks for the last two invariants (see above):
        // the last link must be a relation and its table must be the same as the new link's self table
        assert!(
            {
                let last_link = self.0.last().unwrap();
                matches!(
                    last_link,
                    ColumnPathLink::Relation(RelationLink {
                        column_pairs,
                        ..
                    }) if column_pairs.iter().all(|pair| pair.foreign_column_id.table_id == link.self_table_id())
                )
            },
            "Expected link to point to next table"
        );

        self.0.push(link);

        self
    }

    pub fn join(mut self, tail: Self) -> Self {
        // Assert that the the last link in the path points to the same table as the new link's self table
        // This checks for the last two invariants (see above):
        // the last link must be a relation and its table must be the same as the new link's self table
        assert!(matches!(
            self.0.last().unwrap(),
            ColumnPathLink::Relation(RelationLink {
                column_pairs,
                ..
            }) if column_pairs.iter().all(|pair| pair.foreign_column_id.table_id == tail.0[0].self_table_id())
        ));

        self.0.extend(tail.0);

        self
    }

    #[cfg(test)]
    pub fn from_columns(columns: Vec<ColumnId>, database: &Database) -> Self {
        use crate::{get_mto_relation_for_columns, get_otm_relation_for_columns};

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
                        get_otm_relation_for_columns(&[*column_id], database)
                            .unwrap()
                            .deref(database)
                            .column_path_link()
                    } else {
                        get_mto_relation_for_columns(&[*column_id], database)
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

    /// Alias to be used for the last table in the path
    ///
    /// Picks the linked_table_alias for the penultimate link in the path
    pub fn alias(&self) -> Option<String> {
        for link in self.0.iter().rev() {
            if let ColumnPathLink::Relation(RelationLink {
                linked_table_alias, ..
            }) = link
            {
                return linked_table_alias.clone();
            }
        }
        None
    }
}
