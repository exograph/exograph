// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use exo_sql_core::{
    SchemaStatement,
    column_default::{ColumnAutoincrement, ColumnDefault},
};
use exo_sql_pg::physical_column_type::{
    ArrayColumnType, BlobColumnType, BooleanColumnType, DateColumnType, EnumColumnType, FloatBits,
    FloatColumnType, IntBits, IntColumnType, JsonColumnType, NumericColumnType, PhysicalColumnType,
    StringColumnType, TimeColumnType, TimestampColumnType, UuidColumnType, VectorColumnType,
};
use std::fmt::Write;

/// PostgreSQL DDL generation for column types.
pub trait ColumnTypeSchema {
    fn to_schema(&self, default_value: Option<&ColumnDefault>) -> SchemaStatement;
}

impl ColumnTypeSchema for IntColumnType {
    fn to_schema(&self, default_value: Option<&ColumnDefault>) -> SchemaStatement {
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
}

impl ColumnTypeSchema for StringColumnType {
    fn to_schema(&self, _default_value: Option<&ColumnDefault>) -> SchemaStatement {
        SchemaStatement {
            statement: if let Some(max_length) = self.max_length {
                format!("VARCHAR({max_length})")
            } else {
                "TEXT".to_owned()
            },
            pre_statements: vec![],
            post_statements: vec![],
        }
    }
}

impl ColumnTypeSchema for BooleanColumnType {
    fn to_schema(&self, _default_value: Option<&ColumnDefault>) -> SchemaStatement {
        SchemaStatement {
            statement: "BOOLEAN".to_owned(),
            pre_statements: vec![],
            post_statements: vec![],
        }
    }
}

impl ColumnTypeSchema for FloatColumnType {
    fn to_schema(&self, _default_value: Option<&ColumnDefault>) -> SchemaStatement {
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
}

impl ColumnTypeSchema for NumericColumnType {
    fn to_schema(&self, _default_value: Option<&ColumnDefault>) -> SchemaStatement {
        SchemaStatement {
            statement: {
                if let Some(p) = self.precision {
                    if let Some(s) = self.scale {
                        format!("NUMERIC({p}, {s})")
                    } else {
                        format!("NUMERIC({p})")
                    }
                } else {
                    assert!(self.scale.is_none());
                    "NUMERIC".to_owned()
                }
            },
            pre_statements: vec![],
            post_statements: vec![],
        }
    }
}

impl ColumnTypeSchema for DateColumnType {
    fn to_schema(&self, _default_value: Option<&ColumnDefault>) -> SchemaStatement {
        SchemaStatement {
            statement: "DATE".to_owned(),
            pre_statements: vec![],
            post_statements: vec![],
        }
    }
}

impl ColumnTypeSchema for TimeColumnType {
    fn to_schema(&self, _default_value: Option<&ColumnDefault>) -> SchemaStatement {
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
}

impl ColumnTypeSchema for TimestampColumnType {
    fn to_schema(&self, _default_value: Option<&ColumnDefault>) -> SchemaStatement {
        SchemaStatement {
            statement: {
                let timezone_option = if self.timezone {
                    "WITH TIME ZONE"
                } else {
                    "WITHOUT TIME ZONE"
                };
                let precision_option = if let Some(p) = self.precision {
                    format!("({p})")
                } else {
                    String::default()
                };

                format!("TIMESTAMP{precision_option} {timezone_option}")
            },
            pre_statements: vec![],
            post_statements: vec![],
        }
    }
}

impl ColumnTypeSchema for UuidColumnType {
    fn to_schema(&self, _default_value: Option<&ColumnDefault>) -> SchemaStatement {
        SchemaStatement {
            statement: "uuid".to_owned(),
            pre_statements: vec![],
            post_statements: vec![],
        }
    }
}

impl ColumnTypeSchema for JsonColumnType {
    fn to_schema(&self, _default_value: Option<&ColumnDefault>) -> SchemaStatement {
        SchemaStatement {
            statement: "JSONB".to_owned(),
            pre_statements: vec![],
            post_statements: vec![],
        }
    }
}

impl ColumnTypeSchema for BlobColumnType {
    fn to_schema(&self, _default_value: Option<&ColumnDefault>) -> SchemaStatement {
        SchemaStatement {
            statement: "BYTEA".to_owned(),
            pre_statements: vec![],
            post_statements: vec![],
        }
    }
}

impl ColumnTypeSchema for VectorColumnType {
    fn to_schema(&self, _default_value: Option<&ColumnDefault>) -> SchemaStatement {
        SchemaStatement {
            statement: format!("Vector({})", self.size),
            pre_statements: vec![],
            post_statements: vec![],
        }
    }
}

impl ColumnTypeSchema for EnumColumnType {
    fn to_schema(&self, _default_value: Option<&ColumnDefault>) -> SchemaStatement {
        SchemaStatement {
            statement: self.enum_name.sql_name(),
            pre_statements: vec![],
            post_statements: vec![],
        }
    }
}

impl ColumnTypeSchema for ArrayColumnType {
    fn to_schema(&self, default_value: Option<&ColumnDefault>) -> SchemaStatement {
        let mut underlying_typ = &self.typ;
        let mut dimensions = 1;

        while let Some(array_type) = underlying_typ.as_any().downcast_ref::<ArrayColumnType>() {
            underlying_typ = &array_type.typ;
            dimensions += 1;
        }

        let mut dimensions_part = String::new();
        for _ in 0..dimensions {
            write!(&mut dimensions_part, "[]").unwrap();
        }

        let mut sql_statement =
            as_column_type_schema(underlying_typ.as_ref()).to_schema(default_value);
        sql_statement.statement += &dimensions_part;
        sql_statement
    }
}

exo_sql_pg::downcast_physical_column_type!(as_column_type_schema, ColumnTypeSchema);

/// Extension trait for convenient access to ColumnTypeSchema on `dyn PhysicalColumnType`.
pub trait ColumnTypeSchemaExt {
    fn to_schema(&self, default_value: Option<&ColumnDefault>) -> SchemaStatement;
}

impl ColumnTypeSchemaExt for dyn PhysicalColumnType + '_ {
    fn to_schema(&self, default_value: Option<&ColumnDefault>) -> SchemaStatement {
        as_column_type_schema(self).to_schema(default_value)
    }
}

impl ColumnTypeSchemaExt for Box<dyn PhysicalColumnType> {
    fn to_schema(&self, default_value: Option<&ColumnDefault>) -> SchemaStatement {
        as_column_type_schema(self.as_ref()).to_schema(default_value)
    }
}
