// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use exo_sql_core::Database;

use crate::{ExpressionBuilder, SQLBuilder};

// Re-export the core GroupBy type
pub use exo_sql_core::operation::GroupBy;

impl ExpressionBuilder for GroupBy {
    /// Build expression of the form `GROUP BY <comma-separated-columns>`
    fn build(&self, database: &Database, builder: &mut SQLBuilder) {
        builder.push_str("GROUP BY ");
        let columns = self
            .0
            .iter()
            .map(|column_id| column_id.get_column(database))
            .collect::<Vec<_>>();
        builder.push_elems(database, &columns, ", ");
    }
}
