// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use super::{PhysicalColumnType, PhysicalColumnTypeSerializer};
use crate::{
    SchemaObjectName,
    schema::{column_spec::ColumnDefault, statement::SchemaStatement},
};
use serde::{Deserialize, Serialize};
use std::any::Any;
use std::hash::Hash;
use tokio_postgres::types::Type;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EnumColumnType {
    pub enum_name: SchemaObjectName,
}

impl PhysicalColumnType for EnumColumnType {
    fn type_string(&self) -> String {
        format!("Enum({})", self.enum_name.name)
    }

    fn get_pg_type(&self) -> Type {
        Type::TEXT
    }

    fn to_sql(&self, _default_value: Option<&ColumnDefault>) -> SchemaStatement {
        SchemaStatement {
            statement: self.enum_name.sql_name(),
            pre_statements: vec![],
            post_statements: vec![],
        }
    }

    fn type_name(&self) -> &'static str {
        "Enum"
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

pub struct EnumColumnTypeSerializer;

impl PhysicalColumnTypeSerializer for EnumColumnTypeSerializer {
    fn serialize(&self, column_type: &dyn PhysicalColumnType) -> Result<Vec<u8>, String> {
        column_type
            .as_any()
            .downcast_ref::<EnumColumnType>()
            .ok_or_else(|| "Expected EnumColumnType".to_string())
            .and_then(|t| {
                bincode::serialize(t).map_err(|e| format!("Failed to serialize Enum: {}", e))
            })
    }

    fn deserialize(&self, data: &[u8]) -> Result<Box<dyn PhysicalColumnType>, String> {
        bincode::deserialize::<EnumColumnType>(data)
            .map(|t| Box::new(t) as Box<dyn PhysicalColumnType>)
            .map_err(|e| format!("Failed to deserialize Enum: {}", e))
    }
}
