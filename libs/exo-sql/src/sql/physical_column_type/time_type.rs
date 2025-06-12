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

    fn get_pg_type(&self) -> Type {
        Type::TIME
    }

    fn to_sql(&self, _default_value: Option<&ColumnDefault>) -> SchemaStatement {
        SchemaStatement {
            statement: if let Some(p) = self.precision {
                format!("TIME({p})")
            } else {
                "TIME".to_owned()
            },
            pre_statements: vec![],
            post_statements: vec![],
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

    fn hash_type(&self, state: &mut dyn std::hash::Hasher) {
        state.write(self.type_name().as_bytes());
        if let Some(precision) = &self.precision {
            state.write_usize(*precision);
        }
    }
}

pub fn serialize_time_column_type(column_type: &dyn PhysicalColumnType) -> Result<Vec<u8>, String> {
    column_type
        .as_any()
        .downcast_ref::<TimeColumnType>()
        .ok_or_else(|| "Expected TimeColumnType".to_string())
        .and_then(|t| bincode::serialize(t).map_err(|e| format!("Failed to serialize Time: {}", e)))
}

pub fn deserialize_time_column_type(data: &[u8]) -> Result<Box<dyn PhysicalColumnType>, String> {
    bincode::deserialize::<TimeColumnType>(data)
        .map(|t| Box::new(t) as Box<dyn PhysicalColumnType>)
        .map_err(|e| format!("Failed to deserialize Time: {}", e))
}
