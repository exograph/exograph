use super::{select::Select, sql_operation::SQLOperation, Expression, SQLBuilder};

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

impl<'a> Expression for Cte<'a> {
    fn binding(&self, builder: &mut SQLBuilder) {
        builder.push_str("WITH ");
        builder.push_elems(&self.expressions, ", ");

        builder.push(' ');
        self.select.binding(builder);
    }
}

impl Expression for CteExpression<'_> {
    fn binding(&self, builder: &mut SQLBuilder) {
        builder.push_quoted(&self.name);
        builder.push_str(" AS (");
        self.operation.binding(builder);
        builder.push(')');
    }
}
