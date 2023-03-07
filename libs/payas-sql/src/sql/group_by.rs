use crate::PhysicalColumn;

use super::{Expression, SQLBuilder};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupBy<'a>(pub Vec<&'a PhysicalColumn>);

impl<'a> Expression for GroupBy<'a> {
    fn binding(&self, builder: &mut SQLBuilder) {
        builder.push_str("GROUP BY ");
        builder.push_iter(self.0.iter(), ", ", |builder, elem| {
            elem.binding(builder);
        });
    }
}
