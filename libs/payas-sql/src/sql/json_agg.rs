use crate::Column;

use super::{ExpressionBuilder, SQLBuilder};

#[derive(Debug, PartialEq)]
pub struct JsonAgg<'a>(pub Box<Column<'a>>);

impl<'a> ExpressionBuilder for JsonAgg<'a> {
    fn build(&self, builder: &mut SQLBuilder) {
        // coalesce to return an empty array if we have no matching entities
        builder.push_str("COALESCE(json_agg(");
        self.0.build(builder);
        builder.push_str("), '[]'::json)");
    }
}
