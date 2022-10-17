// use super::column_path::ColumnIdPath;

use core_model::access::AccessContextSelection;
use serde::{Deserialize, Serialize};

use crate::column_path::ColumnIdPath;

/// Access specification for a model
#[derive(Serialize, Deserialize, Debug, Clone)]
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
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum DatabaseAccessPrimitiveExpression {
    ContextSelection(AccessContextSelection), // for example, AuthContext.role
    Column(ColumnIdPath),                     // for example, self.id
    StringLiteral(String),                    // for example, "ROLE_ADMIN"
    BooleanLiteral(bool),                     // for example, true
    NumberLiteral(i64),                       // for example, integer (-13, 0, 300, etc.)
}

/// An expression that can be evaluated to a `Predicate`.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum AccessPredicateExpression<PrimExpr>
where
    PrimExpr: Send + Sync,
{
    LogicalOp(AccessLogicalExpression<PrimExpr>),
    RelationalOp(AccessRelationalOp<PrimExpr>),
    BooleanLiteral(bool),
}

/// Logical operation created from `AccessPredicateExpression`s
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

/// Relational operators expressing a relation between two primitive expressions
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
