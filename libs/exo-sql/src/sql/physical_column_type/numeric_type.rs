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
pub struct NumericColumnType {
    pub precision: Option<usize>,
    pub scale: Option<usize>,
}

impl PhysicalColumnType for NumericColumnType {
    fn type_string(&self) -> String {
        match (self.precision, self.scale) {
            (Some(precision), Some(scale)) => {
                format!("Numeric(precision: {}, scale: {})", precision, scale)
            }
            (Some(precision), None) => format!("Numeric(precision: {})", precision),
            (None, None) => "Numeric".to_string(),
            (None, Some(_)) => unreachable!("scale without precision is not allowed"),
        }
    }

    fn get_pg_type(&self) -> Type {
        Type::NUMERIC
    }

    fn to_sql(&self, _default_value: Option<&ColumnDefault>) -> SchemaStatement {
        SchemaStatement {
            statement: {
                if let Some(p) = self.precision {
                    if let Some(s) = self.scale {
                        format!("NUMERIC({p}, {s})")
                    } else {
                        format!("NUMERIC({p})")
                    }
                } else {
                    assert!(self.scale.is_none()); // can't have a scale and no precision
                    "NUMERIC".to_owned()
                }
            },
            pre_statements: vec![],
            post_statements: vec![],
        }
    }

    fn type_name(&self) -> &'static str {
        "Numeric"
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

pub struct NumericColumnTypeSerializer;

impl PhysicalColumnTypeSerializer for NumericColumnTypeSerializer {
    fn serialize(&self, column_type: &dyn PhysicalColumnType) -> Result<Vec<u8>, String> {
        column_type
            .as_any()
            .downcast_ref::<NumericColumnType>()
            .ok_or_else(|| "Expected NumericColumnType".to_string())
            .and_then(|t| {
                bincode::serde::encode_to_vec(t, bincode::config::standard())
                    .map_err(|e| format!("Failed to serialize Numeric: {}", e))
            })
    }

    fn deserialize(&self, data: &[u8]) -> Result<Box<dyn PhysicalColumnType>, String> {
        let (t, _): (NumericColumnType, _) =
            bincode::serde::decode_from_slice(data, bincode::config::standard())
                .map_err(|e| format!("Failed to deserialize Numeric: {}", e))?;
        Ok(Box::new(t) as Box<dyn PhysicalColumnType>)
    }
}
