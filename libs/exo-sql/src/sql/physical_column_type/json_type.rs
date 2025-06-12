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
pub struct JsonColumnType;

impl PhysicalColumnType for JsonColumnType {
    fn type_string(&self) -> String {
        "Json".to_string()
    }

    fn get_pg_type(&self) -> Type {
        Type::JSONB
    }

    fn to_sql(&self, _default_value: Option<&ColumnDefault>) -> SchemaStatement {
        SchemaStatement {
            statement: "JSONB".to_owned(),
            pre_statements: vec![],
            post_statements: vec![],
        }
    }

    fn type_name(&self) -> &'static str {
        "Json"
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn clone_box(&self) -> Box<dyn PhysicalColumnType> {
        Box::new(self.clone())
    }

    fn equals(&self, other: &dyn PhysicalColumnType) -> bool {
        other.as_any().downcast_ref::<Self>().is_some()
    }

    fn hash_type(&self, state: &mut dyn std::hash::Hasher) {
        state.write(self.type_name().as_bytes());
    }
}

pub fn serialize_json_column_type(column_type: &dyn PhysicalColumnType) -> Result<Vec<u8>, String> {
    column_type
        .as_any()
        .downcast_ref::<JsonColumnType>()
        .ok_or_else(|| "Expected JsonColumnType".to_string())
        .and_then(|t| bincode::serialize(t).map_err(|e| format!("Failed to serialize Json: {}", e)))
}

pub fn deserialize_json_column_type(data: &[u8]) -> Result<Box<dyn PhysicalColumnType>, String> {
    bincode::deserialize::<JsonColumnType>(data)
        .map(|t| Box::new(t) as Box<dyn PhysicalColumnType>)
        .map_err(|e| format!("Failed to deserialize Json: {}", e))
}
