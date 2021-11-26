use super::column_id::ColumnId;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Access {
    pub creation: AccessExpression,
    pub read: AccessExpression,
    pub update: AccessExpression,
    pub delete: AccessExpression,
}

impl Access {
    pub const fn restrictive() -> Self {
        Self {
            creation: AccessExpression::BooleanLiteral(false),
            read: AccessExpression::BooleanLiteral(false),
            update: AccessExpression::BooleanLiteral(false),
            delete: AccessExpression::BooleanLiteral(false),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum AccessExpression {
    ContextSelection(AccessConextSelection), // AuthContext.role
    Column(ColumnId), // self.id (special case of a boolean column such as self.published will be expanded to self.published == true when building an AccessExpression)
    StringLiteral(String), // "ROLE_ADMIN"
    BooleanLiteral(bool), // true as in `self.published == true`
    NumberLiteral(i64), // integer (-13, 0, 300, etc.)
    LogicalOp(AccessLogicalOp),
    RelationalOp(AccessRelationalOp),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum AccessConextSelection {
    Single(String),
    Select(Box<AccessConextSelection>, String),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum AccessLogicalOp {
    Not(Box<AccessExpression>),
    And(Box<AccessExpression>, Box<AccessExpression>),
    Or(Box<AccessExpression>, Box<AccessExpression>),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum AccessRelationalOp {
    Eq(Box<AccessExpression>, Box<AccessExpression>),
    Neq(Box<AccessExpression>, Box<AccessExpression>),
    Lt(Box<AccessExpression>, Box<AccessExpression>),
    Lte(Box<AccessExpression>, Box<AccessExpression>),
    Gt(Box<AccessExpression>, Box<AccessExpression>),
    Gte(Box<AccessExpression>, Box<AccessExpression>),
    In(Box<AccessExpression>, Box<AccessExpression>),
}

impl AccessRelationalOp {
    pub fn sides(&self) -> (&AccessExpression, &AccessExpression) {
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
