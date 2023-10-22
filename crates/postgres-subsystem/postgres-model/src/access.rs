// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use core_plugin_interface::core_model::{
    access::{AccessPredicateExpression, CommonAccessPrimitiveExpression, FunctionCall},
    mapped_arena::SerializableSlabIndex,
};
use exo_sql::PhysicalColumnPath;
use serde::{Deserialize, Serialize};

/// Access specification for a model
#[derive(Serialize, Deserialize, Debug)]
pub struct Access {
    pub creation: SerializableSlabIndex<AccessPredicateExpression<InputAccessPrimitiveExpression>>,
    pub read: SerializableSlabIndex<AccessPredicateExpression<DatabaseAccessPrimitiveExpression>>,
    pub update: UpdateAccessExpression,
    pub delete: SerializableSlabIndex<AccessPredicateExpression<DatabaseAccessPrimitiveExpression>>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct UpdateAccessExpression {
    pub input: SerializableSlabIndex<AccessPredicateExpression<InputAccessPrimitiveExpression>>,
    pub database:
        SerializableSlabIndex<AccessPredicateExpression<DatabaseAccessPrimitiveExpression>>,
}

/// Primitive expression (that doesn't contain any other expressions).
/// Used as sides of `AccessRelationalExpression` to form more complex expressions
/// such as equal and less than.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum DatabaseAccessPrimitiveExpression {
    Column(PhysicalColumnPath, Option<String>), // Column path, for example self.user.id and parameter name (such as "du", default: "self")
    Function(PhysicalColumnPath, FunctionCall<Self>), // Function, for example self.documentUser.some(du => du.id == AuthContext.id && du.read)
    Common(CommonAccessPrimitiveExpression),          // expression shared by all access expressions
}

/// Primitive expressions that can express data input access control rules.
#[derive(Serialize, Deserialize, Debug)]
pub enum InputAccessPrimitiveExpression {
    Path(Vec<String>, Option<String>), // JSON path, for example self.user.id and parameter name (such as "du", default: "self")
    Function(Vec<String>, FunctionCall<Self>), // Function, for example self.documentUser.some(du => du.id == AuthContext.id && du.read)
    Common(CommonAccessPrimitiveExpression),   // expression shared by all access expressions
}
