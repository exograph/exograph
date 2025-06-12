// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use super::PhysicalColumnType;
use crate::schema::{
    column_spec::{ColumnAutoincrement, ColumnDefault},
    statement::SchemaStatement,
};
use serde::{Deserialize, Serialize};
use std::any::Any;
use std::hash::Hash;
use tokio_postgres::types::Type;

/// Number of bits in an integer
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IntBits {
    _16,
    _32,
    _64,
}

impl IntBits {
    pub fn bits(&self) -> usize {
        match self {
            IntBits::_16 => 16,
            IntBits::_32 => 32,
            IntBits::_64 => 64,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct IntColumnType {
    pub bits: IntBits,
}

impl PhysicalColumnType for IntColumnType {
    fn type_string(&self) -> String {
        format!("{}-bit integer", self.bits.bits())
    }

    fn get_pg_type(&self) -> Type {
        match self.bits {
            IntBits::_16 => Type::INT2,
            IntBits::_32 => Type::INT4,
            IntBits::_64 => Type::INT8,
        }
    }

    fn to_sql(&self, default_value: Option<&ColumnDefault>) -> SchemaStatement {
        SchemaStatement {
            statement: {
                if matches!(
                    default_value,
                    Some(ColumnDefault::Autoincrement(ColumnAutoincrement::Serial))
                ) {
                    match self.bits {
                        IntBits::_16 => "SMALLSERIAL",
                        IntBits::_32 => "SERIAL",
                        IntBits::_64 => "BIGSERIAL",
                    }
                } else {
                    match self.bits {
                        IntBits::_16 => "SMALLINT",
                        IntBits::_32 => "INT",
                        IntBits::_64 => "BIGINT",
                    }
                }
            }
            .to_owned(),
            pre_statements: vec![],
            post_statements: vec![],
        }
    }

    fn type_name(&self) -> &'static str {
        "Int"
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
            IntBits::_16 => 16,
            IntBits::_32 => 32,
            IntBits::_64 => 64,
        });
    }
}

pub fn serialize_int_column_type(column_type: &dyn PhysicalColumnType) -> Result<Vec<u8>, String> {
    column_type
        .as_any()
        .downcast_ref::<IntColumnType>()
        .ok_or_else(|| "Expected IntColumnType".to_string())
        .and_then(|t| bincode::serialize(t).map_err(|e| format!("Failed to serialize Int: {}", e)))
}

pub fn deserialize_int_column_type(data: &[u8]) -> Result<Box<dyn PhysicalColumnType>, String> {
    bincode::deserialize::<IntColumnType>(data)
        .map(|t| Box::new(t) as Box<dyn PhysicalColumnType>)
        .map_err(|e| format!("Failed to deserialize Int: {}", e))
}
