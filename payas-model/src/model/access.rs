use super::column_id::ColumnId;

use serde::{Deserialize, Serialize};

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

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum AccessPrimitiveExpression {
    ContextSelection(AccessConextSelection), // AuthContext.role
    Column(ColumnId), // self.id (special case of a boolean column such as self.published will be expanded to self.published == true when building an AccessExpression)
    StringLiteral(String), // "ROLE_ADMIN"
    BooleanLiteral(bool), // true as in `self.published == true`
    NumberLiteral(i64), // integer (-13, 0, 300, etc.)
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum AccessPredicateExpression {
    LogicalOp(AccessLogicalOp),
    RelationalOp(AccessRelationalOp),
    BooleanLiteral(bool),
    // This allows specifying access rule such as `AuthContext.role == "ROLE_ADMIN" || self.published` instead of
    // AuthContext.role == "ROLE_ADMIN" || self.published == true`
    BooleanColumn(ColumnId),
    // Similarly, this allows specifying access rule such as `AuthContext.superUser` instead of `AuthContext.superUser == true`
    BooleanContextSelection(AccessConextSelection),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum AccessConextSelection {
    Single(String),
    Select(Box<AccessConextSelection>, String),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum AccessLogicalOp {
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
