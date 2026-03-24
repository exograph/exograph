// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use super::{PhysicalColumnType, PhysicalColumnTypeSerializer};
use serde::{Deserialize, Serialize};
use std::any::Any;
use std::hash::Hash;

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
}

pub struct IntColumnTypeSerializer;

impl PhysicalColumnTypeSerializer for IntColumnTypeSerializer {
    fn serialize(&self, column_type: &dyn PhysicalColumnType) -> Result<Vec<u8>, String> {
        column_type
            .as_any()
            .downcast_ref::<IntColumnType>()
            .ok_or_else(|| "Expected IntColumnType".to_string())
            .and_then(|t| {
                postcard::to_allocvec(t).map_err(|e| format!("Failed to serialize Int: {}", e))
            })
    }

    fn deserialize(&self, data: &[u8]) -> Result<Box<dyn PhysicalColumnType>, String> {
        let (t, remaining) = postcard::take_from_bytes::<IntColumnType>(data)
            .map_err(|e| format!("Failed to deserialize Int: {}", e))?;
        if !remaining.is_empty() {
            return Err("Did not consume all bytes during deserialization of Int".to_string());
        }
        Ok(Box::new(t) as Box<dyn PhysicalColumnType>)
    }
}
