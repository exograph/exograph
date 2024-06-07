use serde::{Deserialize, Serialize};

use crate::{database_error::DatabaseError, Database};

use super::{ExpressionBuilder, SQLBuilder};

pub const DEFAULT_VECTOR_SIZE: usize = 1536;

pub struct VectorDistance<C>
where
    C: ExpressionBuilder,
{
    lhs: C,
    rhs: C,
    function: VectorDistanceFunction,
}

impl<C: ExpressionBuilder> VectorDistance<C> {
    pub fn new(lhs: C, rhs: C, function: VectorDistanceFunction) -> Self {
        Self { lhs, rhs, function }
    }
}

impl<C: ExpressionBuilder> ExpressionBuilder for VectorDistance<C> {
    fn build(&self, database: &Database, builder: &mut SQLBuilder) {
        self.lhs.build(database, builder);
        builder.push_space();
        self.function.build(database, builder);
        builder.push_space();
        self.rhs.build(database, builder);
        builder.push_str("::vector");
    }
}

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

    pub fn from_model_string(s: &str) -> Result<Self, DatabaseError> {
        match s {
            "l2" => Ok(VectorDistanceFunction::L2),
            "cosine" => Ok(VectorDistanceFunction::Cosine),
            "ip" => Ok(VectorDistanceFunction::InnerProduct),
            _ => Err(DatabaseError::Generic(format!(
                r#"Unknown vector distance function: {s}. Must be either "cosine", "l2", or "ip""#,
            ))),
        }
    }

    pub fn index_kind_str(&self) -> &'static str {
        match self {
            VectorDistanceFunction::L2 => "vector_l2_ops",
            VectorDistanceFunction::Cosine => "vector_cosine_ops",
            VectorDistanceFunction::InnerProduct => "vector_ip_ops",
        }
    }

    pub fn from_db_string(s: &str) -> Result<Self, DatabaseError> {
        match s {
            "vector_l2_ops" => Ok(VectorDistanceFunction::L2),
            "vector_cosine_ops" => Ok(VectorDistanceFunction::Cosine),
            "vector_ip_ops" => Ok(VectorDistanceFunction::InnerProduct),
            _ => Err(DatabaseError::Generic(format!(
                r#"Unknown vector distance function: {s}. Must be either "vector_cosine_ops", "vector_l2_ops", or "vector_ip_ops""#,
            ))),
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
