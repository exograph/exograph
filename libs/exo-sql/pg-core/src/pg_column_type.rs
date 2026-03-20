// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use crate::physical_column_type::{
    ArrayColumnType, BlobColumnType, BooleanColumnType, DateColumnType, EnumColumnType, FloatBits,
    FloatColumnType, IntBits, IntColumnType, JsonColumnType, NumericColumnType, PhysicalColumnType,
    StringColumnType, TimeColumnType, TimestampColumnType, UuidColumnType, VectorColumnType,
};
use tokio_postgres::types::Type;

/// PostgreSQL wire-protocol type mapping for column types.
pub trait PgColumnType {
    /// Returns the PostgreSQL wire-protocol type.
    fn get_pg_type(&self) -> Type;
}

// Helper function to convert a base PostgreSQL type to its array counterpart.
pub fn to_pg_array_type(pg_type: &Type) -> Type {
    match *pg_type {
        Type::INT2 => Type::INT2_ARRAY,
        Type::INT4 => Type::INT4_ARRAY,
        Type::INT8 => Type::INT8_ARRAY,
        Type::TEXT => Type::TEXT_ARRAY,
        Type::JSONB => Type::JSONB_ARRAY,
        Type::FLOAT4 => Type::FLOAT4_ARRAY,
        Type::FLOAT8 => Type::FLOAT8_ARRAY,
        Type::BOOL => Type::BOOL_ARRAY,
        Type::TIMESTAMPTZ => Type::TIMESTAMPTZ_ARRAY,
        Type::TEXT_ARRAY => Type::TEXT_ARRAY,
        Type::VARCHAR => Type::VARCHAR_ARRAY,
        Type::BYTEA => Type::BYTEA_ARRAY,
        Type::UUID => Type::UUID_ARRAY,
        Type::NUMERIC => Type::NUMERIC_ARRAY,
        _ => unimplemented!("Unsupported array type: {:?}", pg_type),
    }
}

impl PgColumnType for IntColumnType {
    fn get_pg_type(&self) -> Type {
        match self.bits {
            IntBits::_16 => Type::INT2,
            IntBits::_32 => Type::INT4,
            IntBits::_64 => Type::INT8,
        }
    }
}

impl PgColumnType for StringColumnType {
    fn get_pg_type(&self) -> Type {
        if self.max_length.is_some() {
            Type::VARCHAR
        } else {
            Type::TEXT
        }
    }
}

impl PgColumnType for BooleanColumnType {
    fn get_pg_type(&self) -> Type {
        Type::BOOL
    }
}

impl PgColumnType for FloatColumnType {
    fn get_pg_type(&self) -> Type {
        match self.bits {
            FloatBits::_24 => Type::FLOAT4,
            FloatBits::_53 => Type::FLOAT8,
        }
    }
}

impl PgColumnType for NumericColumnType {
    fn get_pg_type(&self) -> Type {
        Type::NUMERIC
    }
}

impl PgColumnType for DateColumnType {
    fn get_pg_type(&self) -> Type {
        Type::DATE
    }
}

impl PgColumnType for TimeColumnType {
    fn get_pg_type(&self) -> Type {
        Type::TIME
    }
}

impl PgColumnType for TimestampColumnType {
    fn get_pg_type(&self) -> Type {
        if self.timezone {
            Type::TIMESTAMPTZ
        } else {
            Type::TIMESTAMP
        }
    }
}

impl PgColumnType for UuidColumnType {
    fn get_pg_type(&self) -> Type {
        Type::UUID
    }
}

impl PgColumnType for JsonColumnType {
    fn get_pg_type(&self) -> Type {
        Type::JSONB
    }
}

impl PgColumnType for BlobColumnType {
    fn get_pg_type(&self) -> Type {
        Type::BYTEA
    }
}

impl PgColumnType for VectorColumnType {
    fn get_pg_type(&self) -> Type {
        Type::FLOAT4_ARRAY
    }
}

impl PgColumnType for EnumColumnType {
    fn get_pg_type(&self) -> Type {
        Type::TEXT
    }
}

impl PgColumnType for ArrayColumnType {
    fn get_pg_type(&self) -> Type {
        to_pg_array_type(&as_pg_column_type(self.typ.as_ref()).get_pg_type())
    }
}

crate::downcast_physical_column_type!(as_pg_column_type, PgColumnType);

/// Extension trait for convenient access to PgColumnType methods on `dyn PhysicalColumnType`.
pub trait PgColumnTypeExt {
    fn get_pg_type(&self) -> Type;
}

impl PgColumnTypeExt for dyn PhysicalColumnType + '_ {
    fn get_pg_type(&self) -> Type {
        as_pg_column_type(self).get_pg_type()
    }
}

impl PgColumnTypeExt for Box<dyn PhysicalColumnType> {
    fn get_pg_type(&self) -> Type {
        as_pg_column_type(self.as_ref()).get_pg_type()
    }
}
