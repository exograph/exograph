// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use core_plugin_interface::core_model::access::AccessPredicateExpression;
use core_plugin_interface::core_model::context_type::ContextSelection;
use exo_sql::PhysicalColumnPath;
use serde::{Deserialize, Serialize};

/// Access specification for a model
#[derive(Serialize, Deserialize, Debug)]
pub struct Access {
    pub creation: AccessPredicateExpression<DatabaseAccessPrimitiveExpression>,
    pub read: AccessPredicateExpression<DatabaseAccessPrimitiveExpression>,
    pub update: AccessPredicateExpression<DatabaseAccessPrimitiveExpression>,
    pub delete: AccessPredicateExpression<DatabaseAccessPrimitiveExpression>,
}

impl Access {
    pub const fn restrictive() -> Self {
        Self {
            creation: AccessPredicateExpression::BooleanLiteral(false),
            read: AccessPredicateExpression::BooleanLiteral(false),
            update: AccessPredicateExpression::BooleanLiteral(false),
            delete: AccessPredicateExpression::BooleanLiteral(false),
        }
    }
}

/// Primitive expression (that doesn't contain any other expressions).
/// Used as sides of `AccessRelationalExpression` to form more complex expressions
/// such as equal and less than.
#[derive(Serialize, Deserialize, Debug)]
pub enum DatabaseAccessPrimitiveExpression {
    ContextSelection(ContextSelection), // for example, AuthContext.role
    Column(PhysicalColumnPath),         // for example, self.id
    StringLiteral(String),              // for example, "ADMIN"
    BooleanLiteral(bool),               // for example, true
    NumberLiteral(i64),                 // for example, integer (-13, 0, 300, etc.)
}
