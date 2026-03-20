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

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TimeColumnType {
    pub precision: Option<usize>,
}

impl PhysicalColumnType for TimeColumnType {
    fn type_string(&self) -> String {
        if let Some(precision) = self.precision {
            format!("Time(precision: {})", precision)
        } else {
            "Time".to_string()
        }
    }

    fn type_name(&self) -> &'static str {
        "Time"
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

pub struct TimeColumnTypeSerializer;

impl PhysicalColumnTypeSerializer for TimeColumnTypeSerializer {
    fn serialize(&self, column_type: &dyn PhysicalColumnType) -> Result<Vec<u8>, String> {
        column_type
            .as_any()
            .downcast_ref::<TimeColumnType>()
            .ok_or_else(|| "Expected TimeColumnType".to_string())
            .and_then(|t| {
                postcard::to_allocvec(t).map_err(|e| format!("Failed to serialize Time: {}", e))
            })
    }

    fn deserialize(&self, data: &[u8]) -> Result<Box<dyn PhysicalColumnType>, String> {
        let (t, remaining) = postcard::take_from_bytes::<TimeColumnType>(data)
            .map_err(|e| format!("Failed to deserialize Time: {}", e))?;
        if !remaining.is_empty() {
            return Err("Did not consume all bytes during deserialization of Time".to_string());
        }
        Ok(Box::new(t) as Box<dyn PhysicalColumnType>)
    }
}
