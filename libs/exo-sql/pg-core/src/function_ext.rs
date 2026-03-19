// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use exo_sql_core::physical_column_type::PhysicalColumnTypeExt;
use exo_sql_core::{Database, Function};

use crate::{ExpressionBuilder, SQLBuilder};

impl ExpressionBuilder for Function {
    fn build(&self, database: &Database, builder: &mut SQLBuilder) {
        match self {
            Function::Named {
                function_name,
                column_id,
            } => {
                builder.push_str(function_name);
                builder.push('(');
                let column = column_id.get_column(database);
                column.build(database, builder);
                builder.push(')');
                if column
                    .typ
                    .is::<exo_sql_core::physical_column_type::VectorColumnType>()
                    && function_name != "count"
                {
                    // For vectors, we need to cast the result to a real array (otherwise it will be a string)
                    builder.push_str("::real[]");
                }
            }
            Function::VectorDistance {
                column_id,
                distance_function,
                target,
            } => {
                let column = column_id.get_column(database);
                column.build(database, builder);
                builder.push_space();
                distance_function.build(database, builder);
                builder.push_space();
                builder.push_param(target.param());
                builder.push_str("::vector");
            }
        }
    }
}
