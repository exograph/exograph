use std::sync::Arc;

use super::{Expression, SQLBuilder};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Offset(pub i64);

impl Expression for Offset {
    fn binding(&self, builder: &mut SQLBuilder) {
        builder.push_str(" OFFSET ");
        builder.push_param(Arc::new(self.0))
    }
}
