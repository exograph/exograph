use std::sync::Arc;

use super::{Expression, ParameterBinding};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Offset(pub i64);

impl Expression for Offset {
    fn binding(&self) -> ParameterBinding {
        ParameterBinding::Parameter(Arc::new(self.0))
    }
}
