use crate::PhysicalColumn;

use super::{ExpressionBuilder, SQLBuilder};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupBy<'a>(pub Vec<&'a PhysicalColumn>);

impl<'a> ExpressionBuilder for GroupBy<'a> {
    fn build(&self, builder: &mut SQLBuilder) {
        builder.push_str("GROUP BY ");
        builder.push_elems(&self.0, ", ");
    }
}
