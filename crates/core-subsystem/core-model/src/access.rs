use serde::{Deserialize, Serialize};

/// A path representing context selection such as `AuthContext.role`
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum AccessContextSelection {
    Context(String),                             // for example, `AuthContext`
    Select(Box<AccessContextSelection>, String), // for example, `AuthContext.role`
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
