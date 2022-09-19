pub mod annotation;
pub mod annotation_map;

mod expression;
mod field_type;
mod logical_op;

mod selection;

pub mod typ;
mod util;

use serde::{Deserialize, Serialize};

pub(super) use annotation_map::AnnotationMap;

pub(super) use typ::{PrimitiveType, Type};

use crate::ast::ast_types::NodeTypedness;

pub struct Scope {
    pub enclosing_model: Option<String>,
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
