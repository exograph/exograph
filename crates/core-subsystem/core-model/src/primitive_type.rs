use std::fmt::{Display, Formatter};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PrimitiveType {
    Int,
    Float,
    Decimal,
    String,
    Boolean,
    LocalDate,
    LocalTime,
    LocalDateTime,
    Instant,
    Json,
    Blob,
    Uuid,
    // TODO: This should not be a primitive type, but a type with modifier or some variation of it
    /// An array version of a primitive type.
    Array(Box<PrimitiveType>),

    // TODO: These should not be a primitive types... perhaps another enum `InjectedType`?
    /// Available as an injected dependency to Deno queries and mutations so that the implementation
    /// can execute queries and mutations.
    Claytip,
    /// Similar to Claytip, but also allows queries and mutations with a privilege of another
    /// context.
    ClaytipPriv,
    /// Available to interceptors so that they can get the operation that is being intercepted.
    Interception(String),
}

impl PrimitiveType {
    pub fn name(&self) -> String {
        match &self {
            PrimitiveType::Int => "Int".to_owned(),
            PrimitiveType::Float => "Float".to_owned(),
            PrimitiveType::Decimal => "Decimal".to_owned(),
            PrimitiveType::String => "String".to_owned(),
            PrimitiveType::Boolean => "Boolean".to_owned(),
            PrimitiveType::LocalDate => "LocalDate".to_owned(),
            PrimitiveType::LocalTime => "LocalTime".to_owned(),
            PrimitiveType::LocalDateTime => "LocalDateTime".to_owned(),
            PrimitiveType::Instant => "Instant".to_owned(),
            PrimitiveType::Json => "Json".to_owned(),
            PrimitiveType::Blob => "Blob".to_owned(),
            PrimitiveType::Uuid => "Uuid".to_owned(),
            PrimitiveType::Claytip => "Claytip".to_owned(),
            PrimitiveType::ClaytipPriv => "ClaytipPriv".to_owned(),
            PrimitiveType::Interception(name) => name.to_owned(),
            PrimitiveType::Array(pt) => format!("[{}]", pt.name()),
        }
    }
}

impl Display for PrimitiveType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.name())
    }
}
