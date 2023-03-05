use crate::PhysicalColumn;

use super::{Expression, ExpressionContext, ParameterBinding};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupBy<'a>(pub Vec<&'a PhysicalColumn>);

impl<'a> Expression for GroupBy<'a> {
    fn binding(&self, expression_context: &mut ExpressionContext) -> ParameterBinding {
        let (stmts, params): (Vec<_>, Vec<_>) = self
            .0
            .iter()
            .map(|elem| {
                let column_binding = elem.binding(expression_context);
                (column_binding.stmt, column_binding.params)
            })
            .unzip();

        ParameterBinding::new(
            format!("GROUP BY {}", stmts.join(", ")),
            params.into_iter().flatten().collect(),
        )
    }
}
