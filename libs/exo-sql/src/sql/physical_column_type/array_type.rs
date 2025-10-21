// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use super::{PhysicalColumnType, PhysicalColumnTypeSerializer, to_pg_array_type};
use crate::schema::{column_spec::ColumnDefault, statement::SchemaStatement};
use serde::Serialize;
use std::any::Any;
use std::fmt::Write;
use tokio_postgres::types::Type;

#[derive(Debug)]
pub struct ArrayColumnType {
    pub typ: Box<dyn PhysicalColumnType>,
}

impl ArrayColumnType {
    pub fn new(inner_type: Box<dyn PhysicalColumnType>) -> Self {
        ArrayColumnType { typ: inner_type }
    }
}

impl Clone for ArrayColumnType {
    fn clone(&self) -> Self {
        ArrayColumnType {
            typ: self.typ.clone(),
        }
    }
}

impl PartialEq for ArrayColumnType {
    fn eq(&self, other: &Self) -> bool {
        self.typ.equals(other.typ.as_ref())
    }
}

impl Eq for ArrayColumnType {}

impl PhysicalColumnType for ArrayColumnType {
    fn type_string(&self) -> String {
        format!("Array of [{}]", self.typ.type_string())
    }

    fn get_pg_type(&self) -> Type {
        to_pg_array_type(&self.typ.get_pg_type())
    }

    fn to_sql(&self, default_value: Option<&ColumnDefault>) -> SchemaStatement {
        // 'unwrap' nested arrays all the way to the underlying primitive type
        let mut underlying_typ = &self.typ;
        let mut dimensions = 1;

        while let Some(array_type) = underlying_typ.as_any().downcast_ref::<ArrayColumnType>() {
            underlying_typ = &array_type.typ;
            dimensions += 1;
        }

        // build dimensions
        let mut dimensions_part = String::new();

        for _ in 0..dimensions {
            write!(&mut dimensions_part, "[]").unwrap();
        }

        let mut sql_statement = underlying_typ.to_sql(default_value);
        sql_statement.statement += &dimensions_part;
        sql_statement
    }

    fn type_name(&self) -> &'static str {
        "Array"
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

impl Serialize for ArrayColumnType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("ArrayColumnType", 1)?;
        state.serialize_field("typ", &self.typ)?;
        state.end()
    }
}

pub struct ArrayColumnTypeSerializer;

impl PhysicalColumnTypeSerializer for ArrayColumnTypeSerializer {
    fn serialize(&self, column_type: &dyn PhysicalColumnType) -> Result<Vec<u8>, String> {
        column_type
            .as_any()
            .downcast_ref::<ArrayColumnType>()
            .ok_or_else(|| "Expected ArrayColumnType".to_string())
            .and_then(|t| {
                bincode::serde::encode_to_vec(t, bincode::config::standard())
                    .map_err(|e| format!("Failed to serialize Array: {}", e))
            })
    }

    fn deserialize(&self, data: &[u8]) -> Result<Box<dyn PhysicalColumnType>, String> {
        use super::PHYSICAL_COLUMN_TYPE_REGISTRY;

        #[derive(serde::Deserialize)]
        struct ArrayData {
            typ: super::SerializedPhysicalColumnType,
        }

        let (array_data, _): (ArrayData, _) =
            bincode::serde::decode_from_slice(data, bincode::config::standard())
                .map_err(|e| format!("Failed to deserialize ArrayColumnType structure: {}", e))?;

        // Look up the inner type in the registry
        let entry = PHYSICAL_COLUMN_TYPE_REGISTRY
            .get(array_data.typ.type_name.as_str())
            .ok_or_else(|| format!("Unknown inner type for array: {}", array_data.typ.type_name))?;

        let inner_type = entry
            .deserialize(&array_data.typ.data)
            .map_err(|e| format!("Failed to deserialize inner type: {}", e))?;

        Ok(Box::new(ArrayColumnType::new(inner_type)) as Box<dyn PhysicalColumnType>)
    }
}
