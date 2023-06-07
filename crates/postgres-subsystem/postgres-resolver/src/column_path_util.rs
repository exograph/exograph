// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use exo_sql::{ColumnPathLink, PhysicalColumnPath};

pub fn to_column_path(
    parent_column_path: &Option<PhysicalColumnPath>,
    next_column_path_link: &Option<ColumnPathLink>,
) -> Option<PhysicalColumnPath> {
    match parent_column_path {
        Some(parent_column_path) => match next_column_path_link {
            Some(next_column_path_link) => {
                let path = parent_column_path.clone();
                Some(path.push(next_column_path_link.clone()))
            }
            None => Some(parent_column_path.clone()),
        },
        None => next_column_path_link
            .as_ref()
            .map(|next_column_path_link| PhysicalColumnPath::init(next_column_path_link.clone())),
    }
}
