// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use crate::ast::ast_types::LogicalOp;

use super::{Type, Typed};

impl LogicalOp<Typed> {
    pub fn typ(&self) -> &Type {
        match &self {
            LogicalOp::Not(_, _, typ) => typ,
            LogicalOp::And(_, _, _, typ) => typ,
            LogicalOp::Or(_, _, _, typ) => typ,
        }
    }
}
