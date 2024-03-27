use serde::{Deserialize, Serialize};

use crate::Database;

use super::{ExpressionBuilder, SQLBuilder};

pub const DEFAULT_VECTOR_SIZE: usize = 1536;

#[derive(Debug, PartialEq, Clone, Copy, Serialize, Deserialize, Hash, Eq, Default)]
pub enum VectorDistanceFunction {
    L2,
    #[default]
    Cosine,
    InnerProduct,
}

impl VectorDistanceFunction {
    pub fn model_string(&self) -> &'static str {
        match self {
            VectorDistanceFunction::L2 => "l2",
            VectorDistanceFunction::Cosine => "cosine",
            VectorDistanceFunction::InnerProduct => "ip",
        }
    }

    pub fn from_model_string(s: &str) -> Option<Self> {
        match s {
            "l2" => Some(VectorDistanceFunction::L2),
            "cosine" => Some(VectorDistanceFunction::Cosine),
            "ip" => Some(VectorDistanceFunction::InnerProduct),
            _ => None,
        }
    }

    pub fn index_kind_str(&self) -> &'static str {
        match self {
            VectorDistanceFunction::L2 => "vector_l2_ops",
            VectorDistanceFunction::Cosine => "vector_cosine_ops",
            VectorDistanceFunction::InnerProduct => "vector_ip_ops",
        }
    }

    pub fn from_db_string(s: &str) -> Option<Self> {
        match s {
            "vector_l2_ops" => Some(VectorDistanceFunction::L2),
            "vector_cosine_ops" => Some(VectorDistanceFunction::Cosine),
            "vector_ip_ops" => Some(VectorDistanceFunction::InnerProduct),
            _ => None,
        }
    }
}

impl ExpressionBuilder for VectorDistanceFunction {
    fn build(&self, _database: &Database, builder: &mut SQLBuilder) {
        match self {
            VectorDistanceFunction::L2 => builder.push_str("<->"),
            VectorDistanceFunction::Cosine => builder.push_str("<=>"),
            VectorDistanceFunction::InnerProduct => builder.push_str("<#>"),
        }
    }
}
