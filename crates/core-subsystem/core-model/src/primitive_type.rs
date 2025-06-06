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
    Plain(PrimitiveBaseType),
    Array(Box<PrimitiveType>),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PrimitiveBaseType {
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
}

impl PrimitiveType {
    pub fn name(&self) -> String {
        match &self {
            PrimitiveType::Plain(pt) => pt.name(),
            PrimitiveType::Array(pt) => format!("[{}]", pt.name()),
        }
    }
}

impl PrimitiveBaseType {
    pub fn name(&self) -> String {
        match &self {
            PrimitiveBaseType::Int => "Int".to_owned(),
            PrimitiveBaseType::Float => "Float".to_owned(),
            PrimitiveBaseType::Decimal => "Decimal".to_owned(),
            PrimitiveBaseType::String => "String".to_owned(),
            PrimitiveBaseType::Boolean => "Boolean".to_owned(),
            PrimitiveBaseType::LocalDate => "LocalDate".to_owned(),
            PrimitiveBaseType::LocalTime => "LocalTime".to_owned(),
            PrimitiveBaseType::LocalDateTime => "LocalDateTime".to_owned(),
            PrimitiveBaseType::Instant => "Instant".to_owned(),
            PrimitiveBaseType::Json => "Json".to_owned(),
            PrimitiveBaseType::Blob => "Blob".to_owned(),
            PrimitiveBaseType::Uuid => "Uuid".to_owned(),
            PrimitiveBaseType::Vector => "Vector".to_owned(),
        }
    }

    pub fn is_primitive(name: &str) -> bool {
        matches!(
            name,
            "Int"
                | "Float"
                | "Decimal"
                | "String"
                | "Boolean"
                | "LocalDate"
                | "LocalTime"
                | "LocalDateTime"
                | "Instant"
                | "Json"
                | "Blob"
                | "Uuid"
                | "Vector"
        )
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
    Number(NumberLiteral),
    String(String),
    Boolean(bool),
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum NumberLiteral {
    Int(i64),
    Float(f64),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]

pub enum InjectedType {
    /// Available as an injected dependency to Deno queries and mutations so that the implementation
    /// can execute queries and mutations.
    Exograph,
    /// Similar to Exograph, but also allows queries and mutations with a privilege of another
    /// context.
    ExographPriv,
    /// Available to interceptors so that they can get the operation that is being intercepted.
    Operation(String),
}

impl InjectedType {
    pub fn name(&self) -> String {
        match &self {
            InjectedType::Exograph => "Exograph".to_owned(),
            InjectedType::ExographPriv => "ExographPriv".to_owned(),
            InjectedType::Operation(name) => name.to_owned(),
        }
    }
}
