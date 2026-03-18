// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use exo_sql_core::{Database, VectorDistanceFunction};

use crate::{ExpressionBuilder, SQLBuilder};

pub struct VectorDistance<C>
where
    C: ExpressionBuilder,
{
    lhs: C,
    rhs: C,
    function: VectorDistanceFunction,
}

impl<C: ExpressionBuilder> VectorDistance<C> {
    pub fn new(lhs: C, rhs: C, function: VectorDistanceFunction) -> Self {
        Self { lhs, rhs, function }
    }
}

impl<C: ExpressionBuilder> ExpressionBuilder for VectorDistance<C> {
    fn build(&self, database: &Database, builder: &mut SQLBuilder) {
        self.lhs.build(database, builder);
        builder.push_space();
        self.function.build(database, builder);
        builder.push_space();
        self.rhs.build(database, builder);
        builder.push_str("::vector");
    }
}

impl ExpressionBuilder for VectorDistanceFunction {
    fn build(&self, _database: &Database, builder: &mut SQLBuilder) {
        match self {
            VectorDistanceFunction::L2 => builder.push_str("<->"),
            VectorDistanceFunction::Cosine => builder.push_str("<=>"),
            VectorDistanceFunction::InnerProduct => builder.push_str("<#>"),
        }
    }
}
