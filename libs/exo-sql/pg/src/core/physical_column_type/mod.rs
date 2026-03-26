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

use exo_sql_core::DatabaseError;
use exo_sql_core::physical_column_type::set_physical_column_type_registry;
pub use exo_sql_core::physical_column_type::{
    PhysicalColumnType, PhysicalColumnTypeExt, PhysicalColumnTypeSerializer,
    SerializedPhysicalColumnType,
};
use indexmap::IndexMap;
use regex::Regex;
use std::sync::LazyLock;

/// Macro to generate a downcast dispatch function from `&dyn PhysicalColumnType` to a target trait.
///
/// This avoids duplicating the same 14-branch downcast chain across multiple crates.
/// Usage: `downcast_physical_column_type!(function_name, TargetTrait)`
#[macro_export]
macro_rules! downcast_physical_column_type {
    ($fn_name:ident, $target_trait:path) => {
        pub fn $fn_name(
            typ: &dyn exo_sql_core::physical_column_type::PhysicalColumnType,
        ) -> &dyn $target_trait {
            use $crate::physical_column_type::*;
            let any = typ.as_any();

            if let Some(t) = any.downcast_ref::<IntColumnType>() {
                return t;
            }
            if let Some(t) = any.downcast_ref::<StringColumnType>() {
                return t;
            }
            if let Some(t) = any.downcast_ref::<BooleanColumnType>() {
                return t;
            }
            if let Some(t) = any.downcast_ref::<FloatColumnType>() {
                return t;
            }
            if let Some(t) = any.downcast_ref::<NumericColumnType>() {
                return t;
            }
            if let Some(t) = any.downcast_ref::<DateColumnType>() {
                return t;
            }
            if let Some(t) = any.downcast_ref::<TimeColumnType>() {
                return t;
            }
            if let Some(t) = any.downcast_ref::<TimestampColumnType>() {
                return t;
            }
            if let Some(t) = any.downcast_ref::<UuidColumnType>() {
                return t;
            }
            if let Some(t) = any.downcast_ref::<JsonColumnType>() {
                return t;
            }
            if let Some(t) = any.downcast_ref::<BlobColumnType>() {
                return t;
            }
            if let Some(t) = any.downcast_ref::<VectorColumnType>() {
                return t;
            }
            if let Some(t) = any.downcast_ref::<EnumColumnType>() {
                return t;
            }
            if let Some(t) = any.downcast_ref::<ArrayColumnType>() {
                return t;
            }

            panic!(
                "Unknown PhysicalColumnType: {:?}. All concrete types must implement {}.",
                typ,
                stringify!($target_trait)
            );
        }
    };
}

// Ensure the registry is initialized on first use
static REGISTRY_INIT: LazyLock<()> = LazyLock::new(|| {
    let mut registry = IndexMap::new();

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

    set_physical_column_type_registry(registry);
});

/// Ensure the physical column type and index kind registries are initialized.
/// Must be called before any serialization/deserialization of PhysicalColumn or PhysicalIndex.
pub fn ensure_registry_initialized() {
    LazyLock::force(&REGISTRY_INIT);
    super::pg_schema_types::ensure_index_kind_registry_initialized();
}

/// Create physical column types from PostgreSQL type strings (e.g., "INT", "VARCHAR(255)", "BOOLEAN[]")
pub fn physical_column_type_from_string(
    s: &str,
) -> Result<Box<dyn PhysicalColumnType>, DatabaseError> {
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
