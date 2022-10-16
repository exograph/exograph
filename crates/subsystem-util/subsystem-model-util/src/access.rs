// use super::column_path::ColumnIdPath;

use core_model::access::AccessContextSelection;
use serde::{Deserialize, Serialize};

/// Access specification for a model
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Access {
    pub value: AccessPredicateExpression,
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
pub enum AccessPrimitiveExpression {
    ContextSelection(AccessContextSelection), // for example, AuthContext.role
    StringLiteral(String),                    // for example, "ROLE_ADMIN"
    BooleanLiteral(bool),                     // for example, true
    NumberLiteral(i64),                       // for example, integer (-13, 0, 300, etc.)
}

/// An expression that can be evaluated to a `Predicate`.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum AccessPredicateExpression {
    LogicalOp(AccessLogicalExpression),
    RelationalOp(AccessRelationalOp),
    BooleanLiteral(bool),
}

/// Logical operation created from `AccessPredicateExpression`s
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum AccessLogicalExpression {
    Not(Box<AccessPredicateExpression>),
    And(
        Box<AccessPredicateExpression>,
        Box<AccessPredicateExpression>,
    ),
    Or(
        Box<AccessPredicateExpression>,
        Box<AccessPredicateExpression>,
    ),
}

/// Relational operators expressing a relation between two primitive expressions
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum AccessRelationalOp {
    Eq(
        Box<AccessPrimitiveExpression>,
        Box<AccessPrimitiveExpression>,
    ),
    Neq(
        Box<AccessPrimitiveExpression>,
        Box<AccessPrimitiveExpression>,
    ),
    Lt(
        Box<AccessPrimitiveExpression>,
        Box<AccessPrimitiveExpression>,
    ),
    Lte(
        Box<AccessPrimitiveExpression>,
        Box<AccessPrimitiveExpression>,
    ),
    Gt(
        Box<AccessPrimitiveExpression>,
        Box<AccessPrimitiveExpression>,
    ),
    Gte(
        Box<AccessPrimitiveExpression>,
        Box<AccessPrimitiveExpression>,
    ),
    In(
        Box<AccessPrimitiveExpression>,
        Box<AccessPrimitiveExpression>,
    ),
}

impl AccessRelationalOp {
    pub fn sides(&self) -> (&AccessPrimitiveExpression, &AccessPrimitiveExpression) {
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
