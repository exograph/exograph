// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use exo_sql_pg_core::ParamEquality;
use exo_sql_pg_core::sql_param_container::SQLParamContainer;

use crate::predicate::AbstractPredicate;

// Re-export types that now live in core
pub use exo_sql_core::column_path::{ColumnPathLink, PhysicalColumnPath, RelationLink};

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
    Predicate(Box<AbstractPredicate>),
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
