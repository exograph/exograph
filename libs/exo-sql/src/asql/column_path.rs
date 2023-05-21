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
    ColumnId, Database,
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
    Physical(Vec<PhysicalColumnPathLink>),
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

impl PartialOrd for PhysicalColumnPathLink {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for PhysicalColumnPathLink {
    fn cmp(&self, other: &Self) -> Ordering {
        fn tupled(link: &PhysicalColumnPathLink) -> (ColumnId, Option<ColumnId>) {
            (link.self_column_id, link.linked_column_id)
        }

        tupled(self).cmp(&tupled(other))
    }
}

/// A link in [`ColumnIdPath`] to connect two tables.
/// Contains two columns that link one table to another, which may be used to form a join between two tables
#[derive(Serialize, Deserialize, Debug, Clone, Hash, PartialEq, Eq)]
pub struct PhysicalColumnPathLink {
    /// The column in the current table that is linked to the next table.
    pub self_column_id: ColumnId,
    /// The column in the next table that is linked to the current table. None implies that this is a terminal column (such as artist.name).
    pub linked_column_id: Option<ColumnId>,
}

impl PhysicalColumnPathLink {
    /// Determines if this link is a one-to-many link.
    ///
    /// If the self column is a primary key and the linked column links to a table, then this is a
    /// one-to-many link. For example, when referring from a venue to concerts, the `venue.id` would
    /// be the self column and `concert.venue_id` would be the linked column.
    pub fn is_one_to_many(&self, database: &Database) -> bool {
        let self_column = database.get_column(self.self_column_id);
        self_column.is_pk && self.linked_column_id.is_some()
    }
}
/// A list of path from that represent a relation between two tables
/// For example to reach concert -> concert_artist -> artist -> name,
/// the path would be [(concert.id, concert_artist.concert_id), (concert_artists.artists_id, artist.id), (artist.name, None)]
/// This information could be used to form a join between multiple tables
#[derive(Serialize, Deserialize, Debug, Clone, Hash, PartialEq, Eq)]
pub struct PhysicalColumnPath {
    pub path: Vec<PhysicalColumnPathLink>,
}

impl PhysicalColumnPathLink {
    pub fn new(self_column_id: ColumnId, linked_column_id: Option<ColumnId>) -> Self {
        Self {
            self_column_id,
            linked_column_id,
        }
    }
}
