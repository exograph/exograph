use crate::sql::SQLParam;

use super::{Expression, ExpressionContext, ParameterBinding};

#[derive(Debug, Clone, PartialEq)]
pub struct Offset(pub i64);

impl Expression for Offset {
    fn binding(&self, expression_context: &mut ExpressionContext) -> ParameterBinding {
        let param_index = expression_context.next_param();
        ParameterBinding::new(
            format! {"OFFSET ${}", param_index},
            vec![&self.0 as &dyn SQLParam],
        )
    }
}
