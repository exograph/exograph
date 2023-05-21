// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use exo_sql::{ColumnIdPath, ColumnIdPathLink, ColumnPath};

pub fn to_column_path(
    parent_column_id_path: &Option<ColumnIdPath>,
    next_column_id_path_link: &Option<ColumnIdPathLink>,
) -> ColumnPath {
    let mut path: Vec<_> = match parent_column_id_path {
        Some(parent_column_id_path) => parent_column_id_path.path.clone(),
        None => vec![],
    };

    if let Some(next_column_id_path_link) = next_column_id_path_link {
        path.push(next_column_id_path_link.clone());
    }

    ColumnPath::Physical(path)
}
