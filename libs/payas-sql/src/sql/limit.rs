use std::sync::Arc;

use super::{Expression, ParameterBinding};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Limit(pub i64);

impl Expression for Limit {
    fn binding(&self) -> ParameterBinding {
        ParameterBinding::Parameter(Arc::new(self.0))
    }
}
