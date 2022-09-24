use serde::{Deserialize, Serialize};

use crate::{mapped_arena::SerializableSlabIndex, primitive_type::PrimitiveType};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ContextType {
    pub name: String,
    pub fields: Vec<ContextField>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ContextField {
    pub name: String,
    pub typ: ContextFieldType,
    pub source: ContextSource,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ContextFieldType {
    Optional(Box<ContextFieldType>),
    Reference(PrimitiveType),
    List(Box<ContextFieldType>),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ContextSource {
    pub annotation_name: String,
    pub value: String,
}
