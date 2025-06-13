// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

mod array_type;
mod blob_type;
mod boolean_type;
mod date_type;
mod enum_type;
mod float_type;
mod int_type;
mod json_type;
mod numeric_type;
mod string_type;
mod time_type;
mod timestamp_type;
mod uuid_type;
mod vector_type;

pub use array_type::{ArrayColumnType, ArrayColumnTypeSerializer};
pub use blob_type::{BlobColumnType, BlobColumnTypeSerializer};
pub use boolean_type::{BooleanColumnType, BooleanColumnTypeSerializer};
pub use date_type::{DateColumnType, DateColumnTypeSerializer};
pub use enum_type::{EnumColumnType, EnumColumnTypeSerializer};
pub use float_type::{FloatBits, FloatColumnType, FloatColumnTypeSerializer};
pub use int_type::{IntBits, IntColumnType, IntColumnTypeSerializer};
pub use json_type::{JsonColumnType, JsonColumnTypeSerializer};
pub use numeric_type::{NumericColumnType, NumericColumnTypeSerializer};
pub use string_type::{StringColumnType, StringColumnTypeSerializer};
pub use time_type::{TimeColumnType, TimeColumnTypeSerializer};
pub use timestamp_type::{TimestampColumnType, TimestampColumnTypeSerializer};
pub use uuid_type::{UuidColumnType, UuidColumnTypeSerializer};
pub use vector_type::{VectorColumnType, VectorColumnTypeSerializer};

use crate::database_error::DatabaseError;
use crate::schema::column_spec::ColumnDefault;
use crate::schema::statement::SchemaStatement;
use indexmap::IndexMap;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::any::Any;
use std::fmt::Debug;
use std::sync::LazyLock;
use tokio_postgres::types::Type;

/// Trait that all physical column types must implement
pub trait PhysicalColumnType: Send + Sync + Debug {
    /// Returns a string description of the type
    fn type_string(&self) -> String;

    /// Returns the PostgreSQL type
    fn get_pg_type(&self) -> Type;

    /// Converts to SQL DDL statement
    fn to_sql(&self, default_value: Option<&ColumnDefault>) -> SchemaStatement;

    /// Returns the type name for serialization
    fn type_name(&self) -> &'static str;

    /// Convert to Any for downcasting
    fn as_any(&self) -> &dyn Any;

    /// Clone the type as a boxed trait object
    fn clone_box(&self) -> Box<dyn PhysicalColumnType>;

    /// Check equality with another physical column type
    fn equals(&self, other: &dyn PhysicalColumnType) -> bool;
}

// Implement standard traits directly on Box<dyn PhysicalColumnType>
impl Clone for Box<dyn PhysicalColumnType> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

impl PartialEq for Box<dyn PhysicalColumnType> {
    fn eq(&self, other: &Self) -> bool {
        self.equals(other.as_ref())
    }
}

impl Eq for Box<dyn PhysicalColumnType> {}

// Extension trait to add helper methods to Box<dyn PhysicalColumnType>
pub trait PhysicalColumnTypeExt {
    /// Check if the inner type is of a specific type
    fn is<T: 'static>(&self) -> bool;

    /// Get a reference to the inner type (for compatibility)
    fn inner(&self) -> &dyn PhysicalColumnType;
}

impl PhysicalColumnTypeExt for Box<dyn PhysicalColumnType> {
    fn is<T: 'static>(&self) -> bool {
        self.as_ref().as_any().is::<T>()
    }

    fn inner(&self) -> &dyn PhysicalColumnType {
        self.as_ref()
    }
}

// Helper function to convert postgres array types
pub(crate) fn to_pg_array_type(pg_type: &Type) -> Type {
    match *pg_type {
        Type::INT2 => Type::INT2_ARRAY,
        Type::INT4 => Type::INT4_ARRAY,
        Type::INT8 => Type::INT8_ARRAY,
        Type::TEXT => Type::TEXT_ARRAY,
        Type::JSONB => Type::JSONB_ARRAY,
        Type::FLOAT4 => Type::FLOAT4_ARRAY,
        Type::FLOAT8 => Type::FLOAT8_ARRAY,
        Type::BOOL => Type::BOOL_ARRAY,
        Type::TIMESTAMPTZ => Type::TIMESTAMPTZ_ARRAY,
        Type::TEXT_ARRAY => Type::TEXT_ARRAY,
        Type::VARCHAR => Type::VARCHAR_ARRAY,
        Type::BYTEA => Type::BYTEA_ARRAY,
        Type::UUID => Type::UUID_ARRAY,
        Type::NUMERIC => Type::NUMERIC_ARRAY,
        _ => unimplemented!("Unsupported array type: {:?}", pg_type),
    }
}

// Factory function to create physical column types from strings
fn physical_column_type_from_string(s: &str) -> Result<Box<dyn PhysicalColumnType>, DatabaseError> {
    let s = s.to_uppercase();

    match s.find('[') {
        // If the type contains `[`, then it's an array type
        Some(idx) => {
            let db_type = &s[..idx]; // The underlying data type (e.g. `INT` in `INT[][]`)
            let mut dims = &s[idx..]; // The array brackets (e.g. `[][]` in `INT[][]`)

            // Count how many `[]` exist in `dims` (how many dimensions does this array have)
            let mut count = 0;
            loop {
                if !dims.is_empty() {
                    if dims.len() >= 2 && &dims[0..2] == "[]" {
                        dims = &dims[2..];
                        count += 1;
                    } else {
                        return Err(DatabaseError::Validation(format!("unknown type {s}")));
                    }
                } else {
                    break;
                }
            }

            // Wrap the underlying type with `ArrayColumnType`
            let mut array_type: Box<dyn PhysicalColumnType> = Box::new(ArrayColumnType {
                typ: physical_column_type_from_string(db_type)?,
            });
            for _ in 0..count - 1 {
                array_type = Box::new(ArrayColumnType { typ: array_type });
            }
            Ok(array_type)
        }

        None => Ok(match s.as_str() {
            // TODO: not really correct...
            "SMALLSERIAL" => Box::new(IntColumnType { bits: IntBits::_16 }),
            "SMALLINT" => Box::new(IntColumnType { bits: IntBits::_16 }),
            "INT" => Box::new(IntColumnType { bits: IntBits::_32 }),
            "INTEGER" => Box::new(IntColumnType { bits: IntBits::_32 }),
            "SERIAL" => Box::new(IntColumnType { bits: IntBits::_32 }),
            "BIGINT" => Box::new(IntColumnType { bits: IntBits::_64 }),
            "BIGSERIAL" => Box::new(IntColumnType { bits: IntBits::_64 }),

            "REAL" => Box::new(FloatColumnType {
                bits: FloatBits::_24,
            }),
            "DOUBLE PRECISION" => Box::new(FloatColumnType {
                bits: FloatBits::_53,
            }),

            "UUID" => Box::new(UuidColumnType),
            "TEXT" => Box::new(StringColumnType { max_length: None }),
            "BOOLEAN" => Box::new(BooleanColumnType),
            "JSONB" => Box::new(JsonColumnType),
            "BYTEA" => Box::new(BlobColumnType),
            s => {
                // parse types with arguments
                // TODO: more robust parsing

                let get_num = |s: &str| {
                    s.chars()
                        .filter(|c| c.is_numeric())
                        .collect::<String>()
                        .parse::<usize>()
                        .ok()
                };

                if s.starts_with("CHARACTER VARYING")
                    || s.starts_with("VARCHAR")
                    || s.starts_with("CHAR")
                {
                    Box::new(StringColumnType {
                        max_length: get_num(s),
                    })
                } else if s.starts_with("TIMESTAMP") {
                    Box::new(TimestampColumnType {
                        precision: get_num(s),
                        timezone: s.contains("WITH TIME ZONE"),
                    })
                } else if s.starts_with("TIME") {
                    Box::new(TimeColumnType {
                        precision: get_num(s),
                    })
                } else if s.starts_with("DATE") {
                    Box::new(DateColumnType)
                } else if s.starts_with("NUMERIC") {
                    if s == "NUMERIC" {
                        // NUMERIC without precision/scale parameters
                        Box::new(NumericColumnType {
                            precision: None,
                            scale: None,
                        })
                    } else {
                        // NUMERIC with precision/scale parameters
                        let regex =
                            Regex::new("NUMERIC\\((?P<precision>\\d+),?(?P<scale>\\d+)?\\)")
                                .map_err(|_| {
                                    DatabaseError::Validation("Invalid numeric column spec".into())
                                })?;
                        let captures = regex.captures(s).ok_or_else(|| {
                            DatabaseError::Validation(format!("Invalid numeric column spec: {}", s))
                        })?;

                        let precision = captures
                            .name("precision")
                            .and_then(|s| s.as_str().parse().ok());
                        let scale = captures.name("scale").and_then(|s| s.as_str().parse().ok());

                        Box::new(NumericColumnType { precision, scale })
                    }
                } else {
                    return Err(DatabaseError::Validation(format!("unknown type {s}")));
                }
            }
        }),
    }
}

// Trait for serializing and deserializing physical column types
pub trait PhysicalColumnTypeSerializer: Send + Sync {
    fn serialize(&self, column_type: &dyn PhysicalColumnType) -> Result<Vec<u8>, String>;
    fn deserialize(&self, data: &[u8]) -> Result<Box<dyn PhysicalColumnType>, String>;
}

// Global registry for physical column types
static PHYSICAL_COLUMN_TYPE_REGISTRY: LazyLock<
    IndexMap<&'static str, Box<dyn PhysicalColumnTypeSerializer>>,
> = LazyLock::new(|| {
    let mut registry = IndexMap::new();

    // Register all built-in types
    registry.insert(
        "Int",
        Box::new(IntColumnTypeSerializer) as Box<dyn PhysicalColumnTypeSerializer>,
    );
    registry.insert(
        "String",
        Box::new(StringColumnTypeSerializer) as Box<dyn PhysicalColumnTypeSerializer>,
    );
    registry.insert(
        "Boolean",
        Box::new(BooleanColumnTypeSerializer) as Box<dyn PhysicalColumnTypeSerializer>,
    );
    registry.insert(
        "Timestamp",
        Box::new(TimestampColumnTypeSerializer) as Box<dyn PhysicalColumnTypeSerializer>,
    );
    registry.insert(
        "Date",
        Box::new(DateColumnTypeSerializer) as Box<dyn PhysicalColumnTypeSerializer>,
    );
    registry.insert(
        "Time",
        Box::new(TimeColumnTypeSerializer) as Box<dyn PhysicalColumnTypeSerializer>,
    );
    registry.insert(
        "Json",
        Box::new(JsonColumnTypeSerializer) as Box<dyn PhysicalColumnTypeSerializer>,
    );
    registry.insert(
        "Blob",
        Box::new(BlobColumnTypeSerializer) as Box<dyn PhysicalColumnTypeSerializer>,
    );
    registry.insert(
        "Uuid",
        Box::new(UuidColumnTypeSerializer) as Box<dyn PhysicalColumnTypeSerializer>,
    );
    registry.insert(
        "Vector",
        Box::new(VectorColumnTypeSerializer) as Box<dyn PhysicalColumnTypeSerializer>,
    );
    registry.insert(
        "Float",
        Box::new(FloatColumnTypeSerializer) as Box<dyn PhysicalColumnTypeSerializer>,
    );
    registry.insert(
        "Numeric",
        Box::new(NumericColumnTypeSerializer) as Box<dyn PhysicalColumnTypeSerializer>,
    );
    registry.insert(
        "Enum",
        Box::new(EnumColumnTypeSerializer) as Box<dyn PhysicalColumnTypeSerializer>,
    );
    registry.insert(
        "Array",
        Box::new(ArrayColumnTypeSerializer) as Box<dyn PhysicalColumnTypeSerializer>,
    );

    registry
});

// Individual deserialize functions are now defined in their respective type files

// Serialization wrapper - similar to how PrimitiveBaseType handles it
#[derive(Serialize, Deserialize)]
pub struct SerializedPhysicalColumnType {
    pub type_name: String,
    pub data: Vec<u8>,
}

impl Serialize for Box<dyn PhysicalColumnType> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let type_name = self.type_name();

        // Use the registry to serialize the specific type data
        let entry = PHYSICAL_COLUMN_TYPE_REGISTRY
            .get(type_name)
            .ok_or_else(|| {
                serde::ser::Error::custom(format!("Unknown physical column type: {}", type_name))
            })?;

        let data = entry
            .serialize(self.as_ref())
            .map_err(serde::ser::Error::custom)?;

        let wrapper = SerializedPhysicalColumnType {
            type_name: type_name.to_string(),
            data,
        };

        wrapper.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Box<dyn PhysicalColumnType> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let wrapper = SerializedPhysicalColumnType::deserialize(deserializer)?;

        // Look up the type in the registry
        let entry = PHYSICAL_COLUMN_TYPE_REGISTRY
            .get(wrapper.type_name.as_str())
            .ok_or_else(|| {
                serde::de::Error::custom(format!(
                    "Unknown physical column type: {}",
                    wrapper.type_name
                ))
            })?;

        // Deserialize using the registered function
        entry
            .deserialize(&wrapper.data)
            .map_err(serde::de::Error::custom)
    }
}

// Helper function to create physical column types from strings (now returns Box<dyn PhysicalColumnType> directly)
pub fn physical_column_type_from_string_boxed(
    s: &str,
) -> Result<Box<dyn PhysicalColumnType>, DatabaseError> {
    physical_column_type_from_string(s)
}
