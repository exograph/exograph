use std::sync::Arc;

use super::{Expression, SQLBuilder};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Limit(pub i64);

impl Expression for Limit {
    fn binding(&self, builder: &mut SQLBuilder) {
        builder.push_str(" LIMIT ");
        builder.push_param(Arc::new(self.0))
    }
}
