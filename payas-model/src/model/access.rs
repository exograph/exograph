use super::column_id::ColumnId;

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone)]
pub enum AccessExpression {
    ContextSelection(AccessConextSelection), // AuthContext.role
    Column(ColumnId), // self.id (special case of a boolean column such as self.published will be expanded to self.published == true when building an AccessExpression)
    StringLiteral(String), // "ROLE_ADMIN"
    BooleanLiteral(bool), // true as in `self.published == true`
    NumberLiteral(i64), // integer (-13, 0, 300, etc.)
    LogicalOp(AccessLogicalOp),
    RelationalOp(AccessRelationalOp),
}

#[derive(Debug, Clone)]
pub enum AccessConextSelection {
    Single(String),
    Select(Box<AccessConextSelection>, String),
}

#[derive(Debug, Clone)]
pub enum AccessLogicalOp {
    Not(Box<AccessExpression>),
    And(Box<AccessExpression>, Box<AccessExpression>),
    Or(Box<AccessExpression>, Box<AccessExpression>),
}

#[derive(Debug, Clone)]
pub enum AccessRelationalOp {
    Eq(Box<AccessExpression>, Box<AccessExpression>),
    Neq(Box<AccessExpression>, Box<AccessExpression>),
    // Lt(Box<AccessExpression<'a>>, Box<AccessExpression<'a>>),
    // Lte(Box<AccessExpression<'a>>, Box<AccessExpression<'a>>),
    // Gt(Box<AccessExpression<'a>>, Box<AccessExpression<'a>>),
    // Gte(Box<AccessExpression<'a>>, Box<AccessExpression<'a>>),
}
