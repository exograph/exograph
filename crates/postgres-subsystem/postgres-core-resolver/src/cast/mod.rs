// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use base64::DecodeError;
use common::value::Val;
use exo_sql::{
    Column, ColumnPath, PhysicalColumn, PhysicalColumnType, PhysicalColumnTypeExt,
    SQLParamContainer,
};
use indexmap::IndexMap;
use std::sync::LazyLock;

use super::postgres_execution_error::PostgresExecutionError;
use thiserror::Error;

pub mod array_provider;
pub mod blob_provider;
pub mod bool_provider;
pub mod date_provider;
pub mod enum_provider;
pub mod float_provider;
pub mod int_provider;
pub mod json_provider;
pub mod numeric_provider;
pub mod string_provider;
pub mod time_provider;
pub mod timestamp_provider;
pub mod uuid_provider;
pub mod vector_provider;

use array_provider::ArrayCastProvider;
use blob_provider::BlobCastProvider;
use bool_provider::BoolCastProvider;
use date_provider::DateCastProvider;
use enum_provider::EnumCastProvider;
use float_provider::FloatCastProvider;
use int_provider::IntCastProvider;
use json_provider::JsonCastProvider;
use numeric_provider::NumericCastProvider;
use string_provider::StringCastProvider;
use time_provider::TimeCastProvider;
use timestamp_provider::TimestampCastProvider;
use uuid_provider::UuidCastProvider;
use vector_provider::VectorCastProvider;

#[derive(Debug, Error)]
pub enum CastError {
    #[error("{0}")]
    Generic(String),

    #[error("{0}")]
    Date(String, #[source] chrono::format::ParseError),

    #[error("{0}")]
    Blob(#[from] DecodeError),

    #[error("{0}")]
    Uuid(#[from] uuid::Error),

    #[error("{0}")]
    BigDecimal(String),

    #[error("{0}")]
    Postgres(#[from] exo_sql::database_error::DatabaseError),
}

pub fn literal_column(
    value: &Val,
    associated_column: &PhysicalColumn,
) -> Result<Column, PostgresExecutionError> {
    cast_value(value, associated_column.typ.inner(), false)
        .map(|value| value.map(Column::Param).unwrap_or(Column::Null))
        .map_err(PostgresExecutionError::CastError)
}

pub fn literal_column_path(
    value: &Val,
    destination_type: &dyn PhysicalColumnType,
    unnest: bool,
) -> Result<ColumnPath, PostgresExecutionError> {
    cast_value(value, destination_type, unnest)
        .map(|value| value.map(ColumnPath::Param).unwrap_or(ColumnPath::Null))
        .map_err(PostgresExecutionError::CastError)
}

pub fn cast_value(
    value: &Val,
    destination_type: &dyn PhysicalColumnType,
    unnest: bool,
) -> Result<Option<SQLParamContainer>, CastError> {
    match value {
        Val::Null => Ok(None),
        _ => {
            if let Some(provider) = find_cast_provider(value, destination_type) {
                provider.cast(value, destination_type, unnest)
            } else {
                Err(CastError::Generic(format!(
                    "No suitable cast provider found for value {:?} to destination type {}",
                    value,
                    destination_type.type_name()
                )))
            }
        }
    }
}

/// Trait for providing casting functionality for specific column types
pub trait CastProvider: Send + Sync {
    /// Check if this provider can handle casting the given value to the destination type
    fn suitable(&self, val: &Val, destination_type: &dyn PhysicalColumnType) -> bool;

    /// Cast the value to the destination type
    fn cast(
        &self,
        val: &Val,
        destination_type: &dyn PhysicalColumnType,
        unnest: bool,
    ) -> Result<Option<SQLParamContainer>, CastError>;
}

/// Global registry for cast providers
static CAST_PROVIDER_REGISTRY: LazyLock<IndexMap<&'static str, Box<dyn CastProvider>>> =
    LazyLock::new(|| {
        let mut registry = IndexMap::new();

        // Register all built-in cast providers
        registry.insert("Int", Box::new(IntCastProvider) as Box<dyn CastProvider>);
        registry.insert(
            "Float",
            Box::new(FloatCastProvider) as Box<dyn CastProvider>,
        );
        registry.insert(
            "String",
            Box::new(StringCastProvider) as Box<dyn CastProvider>,
        );
        registry.insert(
            "Boolean",
            Box::new(BoolCastProvider) as Box<dyn CastProvider>,
        );
        registry.insert("Enum", Box::new(EnumCastProvider) as Box<dyn CastProvider>);
        registry.insert(
            "Vector",
            Box::new(VectorCastProvider) as Box<dyn CastProvider>,
        );
        registry.insert("Json", Box::new(JsonCastProvider) as Box<dyn CastProvider>);
        registry.insert("Blob", Box::new(BlobCastProvider) as Box<dyn CastProvider>);
        registry.insert("Uuid", Box::new(UuidCastProvider) as Box<dyn CastProvider>);
        registry.insert("Date", Box::new(DateCastProvider) as Box<dyn CastProvider>);
        registry.insert("Time", Box::new(TimeCastProvider) as Box<dyn CastProvider>);
        registry.insert(
            "Timestamp",
            Box::new(TimestampCastProvider) as Box<dyn CastProvider>,
        );
        registry.insert(
            "Numeric",
            Box::new(NumericCastProvider) as Box<dyn CastProvider>,
        );
        registry.insert(
            "Array",
            Box::new(ArrayCastProvider) as Box<dyn CastProvider>,
        );

        registry
    });

/// Find a suitable cast provider for the given value and destination type
pub fn find_cast_provider(
    val: &Val,
    destination_type: &dyn PhysicalColumnType,
) -> Option<&'static dyn CastProvider> {
    for provider in CAST_PROVIDER_REGISTRY.values() {
        if provider.suitable(val, destination_type) {
            return Some(provider.as_ref());
        }
    }

    None
}
