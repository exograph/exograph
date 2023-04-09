use core_model::access::{AccessContextSelection, AccessPredicateExpression};
use serde::{Deserialize, Serialize};

/// Access specification for a model
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Access {
    pub value: AccessPredicateExpression<ModuleAccessPrimitiveExpression>,
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
pub enum ModuleAccessPrimitiveExpression {
    ContextSelection(AccessContextSelection), // for example, AuthContext.role
    StringLiteral(String),                    // for example, "ROLE_ADMIN"
    BooleanLiteral(bool),                     // for example, true
    NumberLiteral(i64),                       // for example, integer (-13, 0, 300, etc.)
}
