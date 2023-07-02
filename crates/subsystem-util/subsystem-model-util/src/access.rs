// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use core_model::access::{AccessPredicateExpression, CommonAccessPrimitiveExpression};
use serde::{Deserialize, Serialize};

/// Access specification for a model
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Access {
    pub value: AccessPredicateExpression<ModuleAccessPrimitiveExpression>,
}

impl Access {
    pub const fn restrictive() -> Self {
        Self {
            value: AccessPredicateExpression::BooleanLiteral(false),
        }
    }
}

/// Primitive expression (that doesn't contain any other expressions).
/// Used as sides of `AccessRelationalExpression` to form more complex expressions
/// such as equal and less than.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ModuleAccessPrimitiveExpression {
    // Even though we have only one variant here, for symmetry with other subsystems, we model it as an enum
    Common(CommonAccessPrimitiveExpression), // expression shared by all access expressions
}
