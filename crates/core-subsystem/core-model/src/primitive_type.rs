// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::fmt::{Display, Formatter};
use std::sync::LazyLock;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::type_normalization::{BaseType, Type};

/// Trait that all primitive base types must implement
pub trait PrimitiveBaseType: Send + Sync + std::fmt::Debug {
    /// Returns the name of the primitive type
    fn name(&self) -> &'static str;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PrimitiveType {
    Plain(
        #[serde(
            serialize_with = "serialize_primitive",
            deserialize_with = "deserialize_primitive"
        )]
        &'static dyn PrimitiveBaseType,
    ),
    Array(Box<PrimitiveType>),
}

impl PartialEq for PrimitiveType {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (PrimitiveType::Plain(a), PrimitiveType::Plain(b)) => a.name() == b.name(),
            (PrimitiveType::Array(a), PrimitiveType::Array(b)) => a == b,
            _ => false,
        }
    }
}

fn serialize_primitive<S>(
    primitive: &&'static dyn PrimitiveBaseType,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(primitive.name())
}

fn deserialize_primitive<'de, D>(
    deserializer: D,
) -> Result<&'static dyn PrimitiveBaseType, D::Error>
where
    D: Deserializer<'de>,
{
    let name = String::deserialize(deserializer)?;
    PRIMITIVE_REGISTRY
        .get(name.as_str())
        .copied()
        .ok_or_else(|| serde::de::Error::custom(format!("Unknown primitive type: {}", name)))
}

// Macro to define primitive types with minimal boilerplate
macro_rules! define_primitive_type {
    ($type_name:ident, $type_str:literal) => {
        #[derive(Debug)]
        pub struct $type_name;

        impl $type_name {
            pub const NAME: &'static str = $type_str;
        }

        impl PrimitiveBaseType for $type_name {
            fn name(&self) -> &'static str {
                Self::NAME
            }
        }
    };
}

// Define all primitive types using the macro
define_primitive_type!(IntType, "Int");
define_primitive_type!(FloatType, "Float");
define_primitive_type!(DecimalType, "Decimal");
define_primitive_type!(StringType, "String");
define_primitive_type!(BooleanType, "Boolean");
define_primitive_type!(LocalDateType, "LocalDate");
define_primitive_type!(LocalTimeType, "LocalTime");
define_primitive_type!(LocalDateTimeType, "LocalDateTime");
define_primitive_type!(InstantType, "Instant");
define_primitive_type!(JsonType, "Json");
define_primitive_type!(BlobType, "Blob");
define_primitive_type!(UuidType, "Uuid");
define_primitive_type!(VectorType, "Vector");

// Macro to register primitive types in the registry
macro_rules! register_primitive_types {
    ($registry:ident, $($type_name:ident),* $(,)?) => {
        $(
            $registry.insert(
                $type_name::NAME,
                &$type_name as &'static dyn PrimitiveBaseType,
            );
        )*
    };
}

// Global registry for primitive types (uses IndexMap so that the indices in snapshot tests don't change)
pub static PRIMITIVE_REGISTRY: LazyLock<
    indexmap::IndexMap<&'static str, &'static dyn PrimitiveBaseType>,
> = LazyLock::new(|| {
    let mut registry: indexmap::IndexMap<&'static str, &'static dyn PrimitiveBaseType> =
        indexmap::IndexMap::new();

    // Register built-in primitive types
    register_primitive_types!(
        registry,
        BooleanType,
        IntType,
        FloatType,
        DecimalType,
        StringType,
        LocalTimeType,
        LocalDateTimeType,
        LocalDateType,
        InstantType,
        JsonType,
        BlobType,
        UuidType,
        VectorType,
    );

    registry
});

// Convenience constants for commonly used types (TODO: Once the full refactor is done, we can remove these)
pub static INT_TYPE: &'static dyn PrimitiveBaseType = &IntType;
pub static FLOAT_TYPE: &'static dyn PrimitiveBaseType = &FloatType;
pub static DECIMAL_TYPE: &'static dyn PrimitiveBaseType = &DecimalType;
pub static STRING_TYPE: &'static dyn PrimitiveBaseType = &StringType;
pub static BOOLEAN_TYPE: &'static dyn PrimitiveBaseType = &BooleanType;
pub static LOCAL_DATE_TYPE: &'static dyn PrimitiveBaseType = &LocalDateType;
pub static LOCAL_TIME_TYPE: &'static dyn PrimitiveBaseType = &LocalTimeType;
pub static LOCAL_DATE_TIME_TYPE: &'static dyn PrimitiveBaseType = &LocalDateTimeType;
pub static INSTANT_TYPE: &'static dyn PrimitiveBaseType = &InstantType;
pub static JSON_TYPE: &'static dyn PrimitiveBaseType = &JsonType;
pub static BLOB_TYPE: &'static dyn PrimitiveBaseType = &BlobType;
pub static UUID_TYPE: &'static dyn PrimitiveBaseType = &UuidType;
pub static VECTOR_TYPE: &'static dyn PrimitiveBaseType = &VectorType;

impl PrimitiveType {
    pub fn name(&self) -> String {
        match &self {
            PrimitiveType::Plain(pt) => pt.name().to_string(),
            PrimitiveType::Array(pt) => format!("[{}]", pt.name()),
        }
    }

    pub fn is_primitive(name: &str) -> bool {
        PRIMITIVE_REGISTRY.contains_key(name)
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
