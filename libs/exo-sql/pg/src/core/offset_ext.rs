// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::sync::Arc;

use crate::core::sql_param::SQLParamWithType;
use exo_sql_core::{Database, Offset};

use crate::{ExpressionBuilder, SQLBuilder};

impl ExpressionBuilder for Offset {
    /// Build expression of the form `OFFSET <offset>`
    fn build(&self, _database: &Database, builder: &mut SQLBuilder) {
        builder.push_str("OFFSET ");
        builder.push_param(SQLParamWithType {
            param: Arc::new(self.0),
            param_type: tokio_postgres::types::Type::INT8,
            is_array: false,
            enum_type: None,
        });
    }
}
