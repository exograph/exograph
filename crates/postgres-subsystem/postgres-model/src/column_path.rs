// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use serde::{Deserialize, Serialize};

use crate::column_id::ColumnId;

/// The two columns that link one table to another
/// These columns may be used to form a join between two tables
#[derive(Serialize, Deserialize, Debug, Clone, Hash, PartialEq, Eq)]
pub struct ColumnIdPathLink {
    pub self_column_id: ColumnId,
    pub linked_column_id: Option<ColumnId>,
}

/// A list of path from that represent a relation between two tables
/// For example to reach concert -> concert_artist -> artist -> name,
/// the path would be [(concert.id, concert_artist.concert_id), (concert_artists.artists_id, artist.id), (artist.name, None)]
/// This information could be used to form a join between multiple tables
#[derive(Serialize, Deserialize, Debug, Clone, Hash, PartialEq, Eq)]
pub struct ColumnIdPath {
    pub path: Vec<ColumnIdPathLink>,
}

impl ColumnIdPath {
    pub fn leaf_column(&self) -> ColumnId {
        self.path.last().expect("Empty column path").self_column_id
    }
}

impl ColumnIdPathLink {
    pub fn new(self_column_id: ColumnId, linked_column_id: Option<ColumnId>) -> Self {
        Self {
            self_column_id,
            linked_column_id,
        }
    }
}
