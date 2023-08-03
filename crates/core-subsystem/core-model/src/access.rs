// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

//! # Access control expressions
//!
//! Access control expressions describe the access control rules such as `AuthContext.role ==
//! "admin"`, `self.id == AuthContext.id`, `self.publishDate < "2022-08-19"`, and `AuthContext.role
//! == "admin" || AuthContext.role == "moderator"`, etc.
//!
//! Each of these expressions use a primitive expression such as `AuthContext.role` or `self.id` to
//! describe the relevant context and the value to compare against. The meaning of each of the
//! primitive expressions is subsystem-specific. For example, in the Deno subsystem,
//! `AuthContext.role` refers to the role of the authenticated user. In the Postgres subsystem,
//! `self.id` refers to the id of the entity type being accessed.
//!
//! Since these primitives differ between subsystems, the access control expressions are generic
//! over a `PrimExpr` type. For example, in the Deno subsystem, `PrimExpr` is
//! `ModuleAccessPrimitiveExpression` and in the Postgres subsystem, `PrimExpr` is
//! `DatabaseAccessPrimitiveExpression`. This allows each subsystem to define primitive expressions
//! specific to their capability (for example, `DatabaseAccessPrimitiveExpression` contains a
//! `Column` variant).

use serde::{Deserialize, Serialize};

use crate::context_type::ContextSelection;

/// An expression that can be evaluated to a subsystem-specific predicate such as Deno's
/// `ModuleAccessPredicate` and Postgres' `AbstractPredicate`.
///
/// Typically, a system-specific access solver will map this expression to a predicate that can be a
/// boolean value or a residual expression that can be passed down to the the underlying system (for
/// example, a `where` clause to the database query).
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum AccessPredicateExpression<PrimExpr>
where
    PrimExpr: Send + Sync,
{
    LogicalOp(AccessLogicalExpression<PrimExpr>),
    RelationalOp(AccessRelationalOp<PrimExpr>),
    BooleanLiteral(bool),
}

/// A logical expression created from other [`AccessPredicateExpression`]s
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum AccessLogicalExpression<PrimExpr>
where
    PrimExpr: Send + Sync,
{
    Not(Box<AccessPredicateExpression<PrimExpr>>),
    And(
        Box<AccessPredicateExpression<PrimExpr>>,
        Box<AccessPredicateExpression<PrimExpr>>,
    ),
    Or(
        Box<AccessPredicateExpression<PrimExpr>>,
        Box<AccessPredicateExpression<PrimExpr>>,
    ),
}

/// A relational expression between two primitive expressions
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum AccessRelationalOp<PrimExpr>
where
    PrimExpr: Send + Sync,
{
    Eq(Box<PrimExpr>, Box<PrimExpr>),
    Neq(Box<PrimExpr>, Box<PrimExpr>),
    Lt(Box<PrimExpr>, Box<PrimExpr>),
    Lte(Box<PrimExpr>, Box<PrimExpr>),
    Gt(Box<PrimExpr>, Box<PrimExpr>),
    Gte(Box<PrimExpr>, Box<PrimExpr>),
    In(Box<PrimExpr>, Box<PrimExpr>),
}

impl<PrimExpr> AccessRelationalOp<PrimExpr>
where
    PrimExpr: Send + Sync,
{
    /// Get the left and right sides of the relational expression. This allows the caller to operate
    /// on the sides without having to match on the enum.
    pub fn sides(&self) -> (&PrimExpr, &PrimExpr) {
        match self {
            AccessRelationalOp::Eq(left, right) => (left, right),
            AccessRelationalOp::Neq(left, right) => (left, right),
            AccessRelationalOp::Lt(left, right) => (left, right),
            AccessRelationalOp::Lte(left, right) => (left, right),
            AccessRelationalOp::Gt(left, right) => (left, right),
            AccessRelationalOp::Gte(left, right) => (left, right),
            AccessRelationalOp::In(left, right) => (left, right),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum CommonAccessPrimitiveExpression {
    ContextSelection(ContextSelection), // for example, AuthContext.role
    StringLiteral(String),              // for example, "ADMIN"
    BooleanLiteral(bool),               // for example, true
    NumberLiteral(i64),                 // for example, integer (-13, 0, 300, etc.)
}
