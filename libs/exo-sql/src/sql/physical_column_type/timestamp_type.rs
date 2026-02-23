// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use super::{PhysicalColumnType, PhysicalColumnTypeSerializer};
use crate::schema::{column_spec::ColumnDefault, statement::SchemaStatement};
use serde::{Deserialize, Serialize};
use std::any::Any;
use std::hash::Hash;
use tokio_postgres::types::Type;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TimestampColumnType {
    pub timezone: bool,
    pub precision: Option<usize>,
}

impl PhysicalColumnType for TimestampColumnType {
    fn type_string(&self) -> String {
        let timezone_str = if self.timezone { " with timezone" } else { "" };
        let precision_str = if let Some(precision) = self.precision {
            format!(" precision: {}", precision)
        } else {
            String::new()
        };
        format!("Timestamp{}{}", precision_str, timezone_str)
    }

    fn get_pg_type(&self) -> Type {
        if self.timezone {
            Type::TIMESTAMPTZ
        } else {
            Type::TIMESTAMP
        }
    }

    fn to_sql(&self, _default_value: Option<&ColumnDefault>) -> SchemaStatement {
        SchemaStatement {
            statement: {
                let timezone_option = if self.timezone {
                    "WITH TIME ZONE"
                } else {
                    "WITHOUT TIME ZONE"
                };
                let precision_option = if let Some(p) = self.precision {
                    format!("({p})")
                } else {
                    String::default()
                };

                // e.g. "TIMESTAMP(3) WITH TIME ZONE"
                format!("TIMESTAMP{precision_option} {timezone_option}")
            },
            pre_statements: vec![],
            post_statements: vec![],
        }
    }

    fn type_name(&self) -> &'static str {
        "Timestamp"
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn clone_box(&self) -> Box<dyn PhysicalColumnType> {
        Box::new(self.clone())
    }

    fn equals(&self, other: &dyn PhysicalColumnType) -> bool {
        other.as_any().downcast_ref::<Self>() == Some(self)
    }
}

pub struct TimestampColumnTypeSerializer;

impl PhysicalColumnTypeSerializer for TimestampColumnTypeSerializer {
    fn serialize(&self, column_type: &dyn PhysicalColumnType) -> Result<Vec<u8>, String> {
        column_type
            .as_any()
            .downcast_ref::<TimestampColumnType>()
            .ok_or_else(|| "Expected TimestampColumnType".to_string())
            .and_then(|t| {
                postcard::to_allocvec(t)
                    .map_err(|e| format!("Failed to serialize Timestamp: {}", e))
            })
    }

    fn deserialize(&self, data: &[u8]) -> Result<Box<dyn PhysicalColumnType>, String> {
        let (t, remaining) = postcard::take_from_bytes::<TimestampColumnType>(data)
            .map_err(|e| format!("Failed to deserialize Timestamp: {}", e))?;
        if !remaining.is_empty() {
            return Err(
                "Did not consume all bytes during deserialization of Timestamp".to_string(),
            );
        }
        Ok(Box::new(t) as Box<dyn PhysicalColumnType>)
    }
}
