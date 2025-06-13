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

/// Number of bits in the float's mantissa
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FloatBits {
    _24,
    _53,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FloatColumnType {
    pub bits: FloatBits,
}

impl PhysicalColumnType for FloatColumnType {
    fn type_string(&self) -> String {
        match self.bits {
            FloatBits::_24 => "Single precision floating point".to_string(),
            FloatBits::_53 => "Double precision floating point".to_string(),
        }
    }

    fn get_pg_type(&self) -> Type {
        match self.bits {
            FloatBits::_24 => Type::FLOAT4,
            FloatBits::_53 => Type::FLOAT8,
        }
    }

    fn to_sql(&self, _default_value: Option<&ColumnDefault>) -> SchemaStatement {
        SchemaStatement {
            statement: match self.bits {
                FloatBits::_24 => "REAL",
                FloatBits::_53 => "DOUBLE PRECISION",
            }
            .to_owned(),
            pre_statements: vec![],
            post_statements: vec![],
        }
    }

    fn type_name(&self) -> &'static str {
        "Float"
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
        state.write_u8(match self.bits {
            FloatBits::_24 => 24,
            FloatBits::_53 => 53,
        });
    }
}

pub struct FloatColumnTypeSerializer;

impl PhysicalColumnTypeSerializer for FloatColumnTypeSerializer {
    fn serialize(&self, column_type: &dyn PhysicalColumnType) -> Result<Vec<u8>, String> {
        column_type
            .as_any()
            .downcast_ref::<FloatColumnType>()
            .ok_or_else(|| "Expected FloatColumnType".to_string())
            .and_then(|t| {
                bincode::serialize(t).map_err(|e| format!("Failed to serialize Float: {}", e))
            })
    }

    fn deserialize(&self, data: &[u8]) -> Result<Box<dyn PhysicalColumnType>, String> {
        bincode::deserialize::<FloatColumnType>(data)
            .map(|t| Box::new(t) as Box<dyn PhysicalColumnType>)
            .map_err(|e| format!("Failed to deserialize Float: {}", e))
    }
}
