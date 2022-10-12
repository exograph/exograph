use super::{Expression, ExpressionContext, ParameterBinding};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Limit(pub i64);

impl Expression for Limit {
    fn binding(&self, expression_context: &mut ExpressionContext) -> ParameterBinding {
        let param_index = expression_context.next_param();
        ParameterBinding::new(format!("LIMIT ${}", param_index), vec![&self.0])
    }
}
