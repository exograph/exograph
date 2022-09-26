use serde::{Deserialize, Serialize};

use crate::primitive_type::PrimitiveType;

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
    pub value: Option<String>,
}

impl ContextFieldType {
    pub fn primitive_type(&self) -> &PrimitiveType {
        match self {
            ContextFieldType::Optional(underlying) | ContextFieldType::List(underlying) => {
                underlying.primitive_type()
            }
            ContextFieldType::Reference(pt) => pt,
        }
    }
}
