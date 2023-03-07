use std::sync::Arc;

use super::{ExpressionBuilder, SQLBuilder};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Limit(pub i64);

impl ExpressionBuilder for Limit {
    fn build(&self, builder: &mut SQLBuilder) {
        builder.push_str(" LIMIT ");
        builder.push_param(Arc::new(self.0))
    }
}
