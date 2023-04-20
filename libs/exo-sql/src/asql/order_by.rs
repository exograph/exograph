// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use crate::sql::order::Ordering;

use super::column_path::ColumnPath;

/// Represents an abstract order-by clause
#[derive(Debug)]
pub struct AbstractOrderBy<'a>(pub Vec<(ColumnPath<'a>, Ordering)>);

impl<'a> AbstractOrderBy<'a> {
    pub fn column_paths(&self) -> Vec<&ColumnPath<'a>> {
        self.0.iter().map(|(path, _)| path).collect()
    }
}
