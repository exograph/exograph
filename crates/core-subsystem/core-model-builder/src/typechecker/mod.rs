pub mod annotation;
pub mod annotation_map;
pub use annotation_map::AnnotationMap;
pub mod typ;

mod expression;
mod field_type;
mod logical_op;
mod selection;

pub(super) use typ::Type;

use crate::ast::ast_types::NodeTypedness;

use serde::{Deserialize, Serialize};

pub struct Scope {
    pub enclosing_type: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Typed;
impl NodeTypedness for Typed {
    type FieldSelection = Type;
    type RelationalOp = Type;
    type Expr = Type;
    type LogicalOp = Type;
    type Field = Type;
    type Annotations = AnnotationMap;
    type Type = bool;
}
