// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use crate::{Column, Database};

use super::{ExpressionBuilder, SQLBuilder};

/// A JSON aggregation corresponding to the Postgres' `json_agg` function.
#[derive(Debug, PartialEq)]
pub struct JsonAgg(pub Box<Column>);

impl ExpressionBuilder for JsonAgg {
    /// Build expression of the form `COALESCE(json_agg(<column>)), '[]'::json)`. The COALESCE
    /// wrapper ensures that return an empty array if we have no matching entities.
    fn build(&self, database: &Database, builder: &mut SQLBuilder) {
        builder.push_str("COALESCE(json_agg(");
        self.0.build(database, builder);
        builder.push_str("), '[]'::json)");
    }
}
