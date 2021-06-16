use super::column_id::ColumnId;

#[derive(Debug, Clone)]
pub enum AccessExpression {
    ContextSelection(AccessConextSelection), // AuthContext.role
    Column(ColumnId), // self.id (special case of a boolean column such as self.published will be expanded to self.published == true when building an AccessExpression)
    StringLiteral(String), // "ROLE_ADMIN"
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
