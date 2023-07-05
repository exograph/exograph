// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use core_plugin_interface::core_model::access::AccessPredicateExpression;
use core_plugin_interface::core_model::access::CommonAccessPrimitiveExpression;
use exo_sql::PhysicalColumnPath;
use serde::{Deserialize, Serialize};

/// Access specification for a model
#[derive(Serialize, Deserialize, Debug)]
pub struct Access {
    pub creation: AccessPredicateExpression<InputAccessPrimitiveExpression>,
    pub read: AccessPredicateExpression<DatabaseAccessPrimitiveExpression>,
    pub update: UpdateAccessExpression,
    pub delete: AccessPredicateExpression<DatabaseAccessPrimitiveExpression>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct UpdateAccessExpression {
    pub input: AccessPredicateExpression<InputAccessPrimitiveExpression>,
    pub existing: AccessPredicateExpression<DatabaseAccessPrimitiveExpression>,
}

impl UpdateAccessExpression {
    pub const fn restrictive() -> Self {
        Self {
            input: AccessPredicateExpression::BooleanLiteral(false),
            existing: AccessPredicateExpression::BooleanLiteral(false),
        }
    }
}

impl Access {
    pub const fn restrictive() -> Self {
        Self {
            creation: AccessPredicateExpression::BooleanLiteral(false),
            read: AccessPredicateExpression::BooleanLiteral(false),
            update: UpdateAccessExpression::restrictive(),
            delete: AccessPredicateExpression::BooleanLiteral(false),
        }
    }
}

/// Primitive expression (that doesn't contain any other expressions).
/// Used as sides of `AccessRelationalExpression` to form more complex expressions
/// such as equal and less than.
#[derive(Serialize, Deserialize, Debug)]
pub enum DatabaseAccessPrimitiveExpression {
    Column(PhysicalColumnPath), // Column path, for example self.user.id
    Common(CommonAccessPrimitiveExpression), // expression shared by all access expressions
}

/// Primitive expressions that can express data input access control rules.
#[derive(Serialize, Deserialize, Debug)]
pub enum InputAccessPrimitiveExpression {
    Path(Vec<String>),                       // JSON path, for example self.user.id
    Common(CommonAccessPrimitiveExpression), // expression shared by all access expressions
}
