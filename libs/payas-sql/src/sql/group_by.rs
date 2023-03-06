use crate::PhysicalColumn;

use super::{Expression, ParameterBinding};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupBy<'a>(pub Vec<&'a PhysicalColumn>);

impl<'a> Expression for GroupBy<'a> {
    fn binding(&self) -> ParameterBinding {
        let exprs = self
            .0
            .iter()
            .map(|column| ParameterBinding::Column(column))
            .collect();

        ParameterBinding::GroupBy(exprs)
    }
}
