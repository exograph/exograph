// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use crate::sql::order::Ordering;

use super::column_path::PhysicalColumnPath;

/// Represents an abstract order-by clause
#[derive(Debug)]
pub struct AbstractOrderBy(pub Vec<(PhysicalColumnPath, Ordering)>);

impl AbstractOrderBy {
    pub fn column_paths(&self) -> Vec<&PhysicalColumnPath> {
        self.0.iter().map(|(path, _)| path).collect()
    }
}
