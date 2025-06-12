// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use super::PhysicalColumnType;
use crate::schema::{column_spec::ColumnDefault, statement::SchemaStatement};
use serde::{Deserialize, Serialize};
use std::any::Any;
use std::hash::Hash;
use tokio_postgres::types::Type;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct StringColumnType {
    pub max_length: Option<usize>,
}

impl PhysicalColumnType for StringColumnType {
    fn type_string(&self) -> String {
        if let Some(max_length) = self.max_length {
            format!("String(max_length: {})", max_length)
        } else {
            "String".to_string()
        }
    }

    fn get_pg_type(&self) -> Type {
        if self.max_length.is_some() {
            Type::VARCHAR
        } else {
            Type::TEXT
        }
    }

    fn to_sql(&self, _default_value: Option<&ColumnDefault>) -> SchemaStatement {
        SchemaStatement {
            statement: if let Some(max_length) = self.max_length {
                format!("VARCHAR({max_length})")
            } else {
                "TEXT".to_owned()
            },
            pre_statements: vec![],
            post_statements: vec![],
        }
    }

    fn type_name(&self) -> &'static str {
        "String"
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

    fn hash_type(&self, state: &mut dyn std::hash::Hasher) {
        state.write(self.type_name().as_bytes());
        if let Some(max_length) = &self.max_length {
            state.write_usize(*max_length);
        }
    }
}

pub fn serialize_string_column_type(
    column_type: &dyn PhysicalColumnType,
) -> Result<Vec<u8>, String> {
    column_type
        .as_any()
        .downcast_ref::<StringColumnType>()
        .ok_or_else(|| "Expected StringColumnType".to_string())
        .and_then(|t| {
            bincode::serialize(t).map_err(|e| format!("Failed to serialize String: {}", e))
        })
}

pub fn deserialize_string_column_type(data: &[u8]) -> Result<Box<dyn PhysicalColumnType>, String> {
    bincode::deserialize::<StringColumnType>(data)
        .map(|t| Box::new(t) as Box<dyn PhysicalColumnType>)
        .map_err(|e| format!("Failed to deserialize String: {}", e))
}
