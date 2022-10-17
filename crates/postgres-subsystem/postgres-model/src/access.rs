// use super::column_path::ColumnIdPath;

use core_model::access::{AccessContextSelection, AccessPredicateExpression};
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
