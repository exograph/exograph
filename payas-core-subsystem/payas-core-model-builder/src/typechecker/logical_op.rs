use crate::ast::ast_types::LogicalOp;

use super::{Type, Typed};

impl LogicalOp<Typed> {
    pub fn typ(&self) -> &Type {
        match &self {
            LogicalOp::Not(_, _, typ) => typ,
            LogicalOp::And(_, _, _, typ) => typ,
            LogicalOp::Or(_, _, _, typ) => typ,
        }
    }
}
