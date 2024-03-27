use serde::{Deserialize, Serialize};

use crate::Database;

use super::{ExpressionBuilder, SQLBuilder};

#[derive(Debug, PartialEq, Clone, Copy, Serialize, Deserialize, Hash, Eq)]
pub enum VectorDistanceOperator {
    L2,
    Cosine,
    InnerProduct,
}

impl VectorDistanceOperator {
    pub fn index_kind_str(&self) -> &'static str {
        match self {
            VectorDistanceOperator::L2 => "vector_l2_ops",
            VectorDistanceOperator::Cosine => "vector_cosine_ops",
            VectorDistanceOperator::InnerProduct => "vector_ip_ops",
        }
    }

    pub fn from_db_string(s: &str) -> Option<Self> {
        match s {
            "vector_l2_ops" => Some(VectorDistanceOperator::L2),
            "vector_cosine_ops" => Some(VectorDistanceOperator::Cosine),
            "vector_ip_ops" => Some(VectorDistanceOperator::InnerProduct),
            _ => None,
        }
    }
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
