use crate::Column;

use super::{ExpressionBuilder, SQLBuilder};

/// A JSON aggregation corresponding to the Postgres' `json_agg` function.
#[derive(Debug, PartialEq)]
pub struct JsonAgg<'a>(pub Box<Column<'a>>);

impl<'a> ExpressionBuilder for JsonAgg<'a> {
    /// Build expression of the form `COALESCE(json_agg(<column>)), '[]'::json)`. The COALESCE
    /// wrapper ensures that return an empty array if we have no matching entities.
    fn build(&self, builder: &mut SQLBuilder) {
        builder.push_str("COALESCE(json_agg(");
        self.0.build(builder);
        builder.push_str("), '[]'::json)");
    }
}
