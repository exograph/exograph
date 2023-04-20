// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::cmp::Ordering;

use crate::{
    sql::{physical_column::PhysicalColumn, predicate::ParamEquality, SQLParamContainer},
    PhysicalTable,
};

/// A link in [`ColumnPath`] to connect two tables.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct ColumnPathLink<'a> {
    /// The column in the current table that is linked to the next table.
    pub self_column: (&'a PhysicalColumn, &'a PhysicalTable), // We keep the table since a column carries the table name and not the table itself
    /// The column in the next table that is linked to the current table. None implies that this is a terminal column (such as artist.name).
    pub linked_column: Option<(&'a PhysicalColumn, &'a PhysicalTable)>,
}

impl ColumnPathLink<'_> {
    /// Determines if this link is a one-to-many link.
    ///
    /// If the self column is a primary key and the linked column links to a table, then this is a
    /// one-to-many link. For example, when referring from a venue to concerts, the `venue.id` would
    /// be the self column and `concert.venue_id` would be the linked column.
    pub fn is_one_to_many(&self) -> bool {
        self.self_column.0.is_pk && self.linked_column.is_some()
    }
}

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
pub enum ColumnPath<'a> {
    Physical(Vec<ColumnPathLink<'a>>),
    Param(SQLParamContainer),
    Null,
}

impl ParamEquality for ColumnPath<'_> {
    fn param_eq(&self, other: &Self) -> Option<bool> {
        match (self, other) {
            (Self::Param(v1), Self::Param(v2)) => Some(v1 == v2),
            _ => None,
        }
    }
}

impl<'a> PartialOrd for ColumnPathLink<'a> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<'a> Ord for ColumnPathLink<'a> {
    fn cmp(&self, other: &Self) -> Ordering {
        fn tupled<'a>(
            link: &'a ColumnPathLink,
        ) -> (&'a str, &'a str, Option<&'a str>, Option<&'a str>) {
            (
                &link.self_column.0.name,
                &link.self_column.1.name,
                link.linked_column.map(|c| c.0.name.as_str()),
                link.linked_column.map(|c| c.1.name.as_str()),
            )
        }

        tupled(self).cmp(&tupled(other))
    }
}
