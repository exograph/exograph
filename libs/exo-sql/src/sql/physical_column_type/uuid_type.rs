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
pub struct UuidColumnType;

impl PhysicalColumnType for UuidColumnType {
    fn type_string(&self) -> String {
        "Uuid".to_string()
    }

    fn get_pg_type(&self) -> Type {
        Type::UUID
    }

    fn to_sql(&self, _default_value: Option<&ColumnDefault>) -> SchemaStatement {
        SchemaStatement {
            statement: "uuid".to_owned(),
            pre_statements: vec![],
            post_statements: vec![],
        }
    }

    fn type_name(&self) -> &'static str {
        "Uuid"
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
}

pub struct UuidColumnTypeSerializer;

impl PhysicalColumnTypeSerializer for UuidColumnTypeSerializer {
    fn serialize(&self, column_type: &dyn PhysicalColumnType) -> Result<Vec<u8>, String> {
        column_type
            .as_any()
            .downcast_ref::<UuidColumnType>()
            .ok_or_else(|| "Expected UuidColumnType".to_string())
            .and_then(|t| {
                bincode::serde::encode_to_vec(t, bincode::config::standard())
                    .map_err(|e| format!("Failed to serialize Uuid: {}", e))
            })
    }

    fn deserialize(&self, data: &[u8]) -> Result<Box<dyn PhysicalColumnType>, String> {
        let (t, _): (UuidColumnType, _) =
            bincode::serde::decode_from_slice(data, bincode::config::standard())
                .map_err(|e| format!("Failed to deserialize Uuid: {}", e))?;
        Ok(Box::new(t) as Box<dyn PhysicalColumnType>)
    }
}
