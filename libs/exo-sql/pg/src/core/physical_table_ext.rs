// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use crate::{ExpressionBuilder, SQLBuilder};
use exo_sql_core::{Database, PhysicalTable};

impl ExpressionBuilder for PhysicalTable {
    /// Build a table reference for the `<table>`.
    fn build(&self, _database: &Database, builder: &mut SQLBuilder) {
        builder.push_table(&self.name);
    }
}
