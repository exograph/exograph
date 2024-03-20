use crate::Database;

use super::{ExpressionBuilder, SQLBuilder};

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum VectorDistanceOperator {
    L2,
    Cosine,
    InnerProduct,
}

impl ExpressionBuilder for VectorDistanceOperator {
    fn build(&self, _database: &Database, builder: &mut SQLBuilder) {
        match self {
            VectorDistanceOperator::L2 => builder.push_str("<->"),
            VectorDistanceOperator::Cosine => builder.push_str("<=>"),
            VectorDistanceOperator::InnerProduct => builder.push_str("<#>"),
        }
    }
}
