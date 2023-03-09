use std::sync::Arc;

use super::{ExpressionBuilder, SQLBuilder};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Offset(pub i64);

impl ExpressionBuilder for Offset {
    /// Build expression of the form `OFFSET <offset>`
    fn build(&self, builder: &mut SQLBuilder) {
        builder.push_str("OFFSET ");
        builder.push_param(Arc::new(self.0))
    }
}
