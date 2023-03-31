use super::{select::Select, sql_operation::SQLOperation, ExpressionBuilder, SQLBuilder};

/// A query with common table expressions of the form `WITH <expressions> <select>`.
#[derive(Debug)]
pub struct WithQuery<'a> {
    /// The "WITH" expressions
    pub expressions: Vec<CteExpression<'a>>,
    /// The select statement
    pub select: Select<'a>,
}

/// A common table expression of the form `<name> AS (<operation>)`.
#[derive(Debug)]
pub struct CteExpression<'a> {
    /// The name of the expression
    pub name: String,
    /// The SQL operation to be bound to the name
    pub operation: SQLOperation<'a>,
}

impl<'a> ExpressionBuilder for WithQuery<'a> {
    /// Build a CTE for the `WITH <expressions> <select>` syntax.
    fn build(&self, builder: &mut SQLBuilder) {
        builder.push_str("WITH ");
        builder.push_elems(&self.expressions, ", ");
        builder.push_space();
        self.select.build(builder);
    }
}

impl ExpressionBuilder for CteExpression<'_> {
    /// Build a CTE expression for the `<name> AS (<operation>)` syntax.
    fn build(&self, builder: &mut SQLBuilder) {
        builder.push_identifier(&self.name);
        builder.push_str(" AS (");
        self.operation.build(builder);
        builder.push(')');
    }
}
