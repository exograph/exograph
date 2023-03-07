use super::{select::Select, sql_operation::SQLOperation, ExpressionBuilder, SQLBuilder};

#[derive(Debug)]
pub struct Cte<'a> {
    pub expressions: Vec<CteExpression<'a>>,
    pub select: Select<'a>,
}

#[derive(Debug)]
pub struct CteExpression<'a> {
    pub name: String,
    pub operation: SQLOperation<'a>,
}

impl<'a> ExpressionBuilder for Cte<'a> {
    /// Build a CTE for the `WITH <expressions> <select>` syntax.
    fn build(&self, builder: &mut SQLBuilder) {
        builder.push_str("WITH ");
        builder.push_elems(&self.expressions, ", ");
        builder.push(' ');
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
