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
    Array(Box<PrimitiveType>),
    // TODO: These should not be a primitive types... perhaps another enum `InjectedType`?
    Claytip,
    ClaytipPriv,
    Interception(String), // Types such as "Operation" that an interceptor is passed to
}

impl PrimitiveType {
    pub fn name(&self) -> String {
        if let PrimitiveType::Array(pt) = &self {
            return format!("[{}]", pt.name());
        }

        match &self {
            PrimitiveType::Int => "Int",
            PrimitiveType::Float => "Float",
            PrimitiveType::Decimal => "Decimal",
            PrimitiveType::String => "String",
            PrimitiveType::Boolean => "Boolean",
            PrimitiveType::LocalDate => "LocalDate",
            PrimitiveType::LocalTime => "LocalTime",
            PrimitiveType::LocalDateTime => "LocalDateTime",
            PrimitiveType::Instant => "Instant",
            PrimitiveType::Json => "Json",
            PrimitiveType::Blob => "Blob",
            PrimitiveType::Uuid => "Uuid",
            PrimitiveType::Claytip => "Claytip",
            PrimitiveType::ClaytipPriv => "ClaytipPriv",
            PrimitiveType::Interception(name) => name,
            PrimitiveType::Array(_) => panic!(),
        }
        .to_owned()
    }
}

impl Display for PrimitiveType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.name())
    }
}
