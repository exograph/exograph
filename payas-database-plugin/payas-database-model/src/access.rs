// use super::column_path::ColumnIdPath;

use serde::{Deserialize, Serialize};

use crate::column_path::ColumnIdPath;

/// Access specification for a model
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Access {
    pub creation: AccessPredicateExpression,
    pub read: AccessPredicateExpression,
    pub update: AccessPredicateExpression,
    pub delete: AccessPredicateExpression,
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
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum AccessPrimitiveExpression {
    ContextSelection(AccessContextSelection), // for example, AuthContext.role
    Column(ColumnIdPath),                     // for example, self.id
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
    // This allows specifying access rule such as `self.published` instead of `self.published == true`
    BooleanColumn(ColumnIdPath),
    // Similarly, this allows specifying access rule such as `AuthContext.superUser` instead of `AuthContext.superUser == true`
    BooleanContextSelection(AccessContextSelection),
}

/// A path representing context selection such as `AuthContext.role`
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum AccessContextSelection {
    Context(String),                             // for example, `AuthContext`
    Select(Box<AccessContextSelection>, String), // for example, `AuthContext.role`
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
