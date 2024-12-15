// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::sync::Arc;

use tokio_postgres::types::Type;

use crate::Database;

use super::{ExpressionBuilder, SQLBuilder};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Offset(pub i64);

impl ExpressionBuilder for Offset {
    /// Build expression of the form `OFFSET <offset>`
    fn build(&self, _database: &Database, builder: &mut SQLBuilder) {
        builder.push_str("OFFSET ");
        builder.push_param((Arc::new(self.0), Type::INT8, false))
    }
}
