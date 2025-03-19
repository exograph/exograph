// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::fmt::{Display, Formatter};

use serde::{Deserialize, Serialize};

use crate::type_normalization::{BaseType, Type};

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
    Vector,
    // TODO: This should not be a primitive type, but a type with modifier or some variation of it
    /// An array version of a primitive type.
    Array(Box<PrimitiveType>),

    // TODO: These should not be a primitive types... perhaps another enum `InjectedType`?
    /// Available as an injected dependency to Deno queries and mutations so that the implementation
    /// can execute queries and mutations.
    Exograph,
    /// Similar to Exograph, but also allows queries and mutations with a privilege of another
    /// context.
    ExographPriv,
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
            PrimitiveType::Vector => "Vector".to_owned(),
            PrimitiveType::Exograph => "Exograph".to_owned(),
            PrimitiveType::ExographPriv => "ExographPriv".to_owned(),
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

pub fn vector_introspection_base_type() -> BaseType {
    BaseType::List(Box::new(Type {
        base: BaseType::Leaf("Float".to_string()),
        nullable: false,
    }))
}

pub fn vector_introspection_type(optional: bool) -> Type {
    Type {
        base: vector_introspection_base_type(),
        nullable: optional,
    }
}

// TODO: We should refactor `PrimitiveValue` along with `Val` to be a single enum
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PrimitiveValue {
    Int(i64),
    String(String),
    Boolean(bool),
}
