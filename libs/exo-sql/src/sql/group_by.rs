// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use crate::PhysicalColumn;

use super::{ExpressionBuilder, SQLBuilder};

/// A group by clause
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupBy<'a>(pub Vec<&'a PhysicalColumn>);

impl<'a> ExpressionBuilder for GroupBy<'a> {
    /// Build expression of the form `GROUP BY <comma-separated-columns>`
    fn build(&self, builder: &mut SQLBuilder) {
        builder.push_str("GROUP BY ");
        builder.push_elems(&self.0, ", ");
    }
}
