// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use crate::{
    Database, ManyToOneId, OneToManyId, SchemaObjectName, TableId,
    database_error::DatabaseError,
    schema::column_spec::{ColumnAutoincrement, ColumnDefault},
};

use super::{ExpressionBuilder, SQLBuilder};
use regex::Regex;
use serde::{Deserialize, Serialize};
use tokio_postgres::types::Type;

/// A column in a physical table
#[derive(Serialize, Deserialize, PartialEq, Eq, Hash, Clone)]
pub struct PhysicalColumn {
    /// The name of the table this column belongs to
    pub table_id: TableId,
    /// The name of the column
    pub name: String,
    /// The type of the column
    pub typ: PhysicalColumnType,
    /// Is this column a part of the PK for the table
    pub is_pk: bool,
    /// should this type have a NOT NULL constraint or not?
    pub is_nullable: bool,

    /// optional names for unique constraints that this column is a part of
    pub unique_constraints: Vec<String>,

    /// optional default value for this column
    pub default_value: Option<ColumnDefault>,
    pub update_sync: bool,

    /// A name that can be used to group columns together (for example to generate a foreign key constraint name for composite primary keys)
    pub group_name: Option<String>,
}

/// Simpler implementation of Debug for PhysicalColumn.
///
/// The derived implementation of Debug for PhysicalColumn is not very useful, since it includes
/// every field of the struct and obscures the actual useful information. This implementation only
/// prints the table name and column name.
impl std::fmt::Debug for PhysicalColumn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&format!(
            "Column: {}.{}",
            &self.table_id.arr_idx(),
            &self.name
        ))
    }
}

impl PhysicalColumn {
    pub fn get_table_name(&self, database: &Database) -> SchemaObjectName {
        database.get_table(self.table_id).name.clone()
    }

    pub fn get_sequence_name(&self) -> Option<SchemaObjectName> {
        match &self.default_value {
            Some(ColumnDefault::Autoincrement(ColumnAutoincrement::Sequence { name })) => {
                Some(name.clone())
            }
            _ => None,
        }
    }
}

impl ExpressionBuilder for PhysicalColumn {
    fn build(&self, database: &Database, builder: &mut SQLBuilder) {
        let table = database.get_table(self.table_id);
        builder.push_table_prefix(&table.name);
        builder.push_identifier(&self.name)
    }
}

/// The type of a column in a physical table to include more precise information than just the type
/// name.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub enum PhysicalColumnType {
    Int {
        bits: IntBits,
    },
    String {
        max_length: Option<usize>,
    },
    Boolean,
    Timestamp {
        timezone: bool,
        precision: Option<usize>,
    },
    Date,
    Time {
        precision: Option<usize>,
    },
    Json,
    Blob,
    Uuid,
    Vector {
        size: usize,
    },
    Array {
        typ: Box<PhysicalColumnType>,
    },
    Float {
        bits: FloatBits,
    },
    Numeric {
        precision: Option<usize>,
        scale: Option<usize>,
    },
    Enum {
        enum_name: SchemaObjectName,
    },
}

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

/// Number of bits in the float's mantissa.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FloatBits {
    _24,
    _53,
}

impl PhysicalColumnType {
    pub fn type_string(&self) -> String {
        match self {
            PhysicalColumnType::Int { bits } => format!("{}-bit integer", bits.bits()),
            PhysicalColumnType::String { max_length } => {
                format!("String of max length {max_length:?}")
            }
            PhysicalColumnType::Boolean => "Boolean".to_string(),
            PhysicalColumnType::Timestamp {
                timezone,
                precision,
            } => {
                format!("Timestamp with timezone: {timezone:?}, precision: {precision:?}")
            }
            PhysicalColumnType::Date => "Date".to_string(),
            PhysicalColumnType::Time { precision } => {
                format!("Time with precision: {precision:?}")
            }
            PhysicalColumnType::Json => "Json".to_string(),
            PhysicalColumnType::Blob => "Blob".to_string(),
            PhysicalColumnType::Uuid => "Uuid".to_string(),
            PhysicalColumnType::Vector { size } => format!("Vector of size {size:?}"),
            PhysicalColumnType::Array { typ } => format!("Array of {typ:?}"),
            PhysicalColumnType::Float { bits } => match bits {
                FloatBits::_24 => "Single precision floating point".to_string(),
                FloatBits::_53 => "Double precision floating point".to_string(),
            },
            PhysicalColumnType::Numeric { precision, scale } => {
                format!("Numeric with precision: {precision:?}, scale: {scale:?}")
            }
            PhysicalColumnType::Enum { enum_name } => {
                format!("Enum with type name: {enum_name:?}")
            }
        }
    }
    /// Create a new physical column type given the SQL type string. This is used to reverse-engineer
    /// a database schema to a Exograph model.
    pub fn from_string(s: &str) -> Result<PhysicalColumnType, DatabaseError> {
        let s = s.to_uppercase();

        match s.find('[') {
            // If the type contains `[`, then it's an array type
            Some(idx) => {
                let db_type = &s[..idx]; // The underlying data type (e.g. `INT` in `INT[][]`)
                let mut dims = &s[idx..]; // The array brackets (e.g. `[][]` in `INT[][]`)

                // Count how many `[]` exist in `dims` (how many dimensions does this array have)
                let mut count = 0;
                loop {
                    if !dims.is_empty() {
                        if dims.len() >= 2 && &dims[0..2] == "[]" {
                            dims = &dims[2..];
                            count += 1;
                        } else {
                            return Err(DatabaseError::Validation(format!("unknown type {s}")));
                        }
                    } else {
                        break;
                    }
                }

                // Wrap the underlying type with `PhysicalColumnType::Array`
                let mut array_type = PhysicalColumnType::Array {
                    typ: Box::new(PhysicalColumnType::from_string(db_type)?),
                };
                for _ in 0..count - 1 {
                    array_type = PhysicalColumnType::Array {
                        typ: Box::new(array_type),
                    };
                }
                Ok(array_type)
            }

            None => Ok(match s.as_str() {
                // TODO: not really correct...
                "SMALLSERIAL" => PhysicalColumnType::Int { bits: IntBits::_16 },
                "SMALLINT" => PhysicalColumnType::Int { bits: IntBits::_16 },
                "INT" => PhysicalColumnType::Int { bits: IntBits::_32 },
                "INTEGER" => PhysicalColumnType::Int { bits: IntBits::_32 },
                "SERIAL" => PhysicalColumnType::Int { bits: IntBits::_32 },
                "BIGINT" => PhysicalColumnType::Int { bits: IntBits::_64 },
                "BIGSERIAL" => PhysicalColumnType::Int { bits: IntBits::_64 },

                "REAL" => PhysicalColumnType::Float {
                    bits: FloatBits::_24,
                },
                "DOUBLE PRECISION" => PhysicalColumnType::Float {
                    bits: FloatBits::_53,
                },

                "UUID" => PhysicalColumnType::Uuid,
                "TEXT" => PhysicalColumnType::String { max_length: None },
                "BOOLEAN" => PhysicalColumnType::Boolean,
                "JSONB" => PhysicalColumnType::Json,
                s => {
                    // parse types with arguments
                    // TODO: more robust parsing

                    let get_num = |s: &str| {
                        s.chars()
                            .filter(|c| c.is_numeric())
                            .collect::<String>()
                            .parse::<usize>()
                            .ok()
                    };

                    if s.starts_with("CHARACTER VARYING")
                        || s.starts_with("VARCHAR")
                        || s.starts_with("CHAR")
                    {
                        PhysicalColumnType::String {
                            max_length: get_num(s),
                        }
                    } else if s.starts_with("TIMESTAMP") {
                        PhysicalColumnType::Timestamp {
                            precision: get_num(s),
                            timezone: s.contains("WITH TIME ZONE"),
                        }
                    } else if s.starts_with("TIME") {
                        PhysicalColumnType::Time {
                            precision: get_num(s),
                        }
                    } else if s.starts_with("DATE") {
                        PhysicalColumnType::Date
                    } else if s.starts_with("NUMERIC") {
                        let regex =
                            Regex::new("NUMERIC\\((?P<precision>\\d+),?(?P<scale>\\d+)?\\)")
                                .map_err(|_| {
                                    DatabaseError::Validation("Invalid numeric column spec".into())
                                })?;
                        let captures = regex.captures(s).unwrap();

                        let precision = captures
                            .name("precision")
                            .and_then(|s| s.as_str().parse().ok());
                        let scale = captures.name("scale").and_then(|s| s.as_str().parse().ok());

                        PhysicalColumnType::Numeric { precision, scale }
                    } else {
                        return Err(DatabaseError::Validation(format!("unknown type {s}")));
                    }
                }
            }),
        }
    }

    pub fn get_pg_type(&self) -> Type {
        match &self {
            PhysicalColumnType::Int { bits } => match bits {
                IntBits::_16 => Type::INT2,
                IntBits::_32 => Type::INT4,
                IntBits::_64 => Type::INT8,
            },
            PhysicalColumnType::String { max_length } => {
                if max_length.is_some() {
                    Type::VARCHAR
                } else {
                    Type::TEXT
                }
            }
            PhysicalColumnType::Boolean => Type::BOOL,
            PhysicalColumnType::Timestamp { timezone, .. } => {
                if *timezone {
                    Type::TIMESTAMPTZ
                } else {
                    Type::TIMESTAMP
                }
            }
            PhysicalColumnType::Date => Type::DATE,
            PhysicalColumnType::Time { .. } => Type::TIME,
            PhysicalColumnType::Json => Type::JSONB,
            PhysicalColumnType::Blob => Type::BYTEA,
            PhysicalColumnType::Uuid => Type::UUID,
            PhysicalColumnType::Array { typ } => to_pg_array_type(&typ.get_pg_type()),
            PhysicalColumnType::Float { bits } => match bits {
                FloatBits::_24 => Type::FLOAT4,
                FloatBits::_53 => Type::FLOAT8,
            },
            PhysicalColumnType::Numeric { .. } => Type::NUMERIC,
            PhysicalColumnType::Vector { .. } => Type::FLOAT4_ARRAY,
            PhysicalColumnType::Enum { .. } => Type::TEXT,
        }
    }
}

pub(crate) fn to_pg_array_type(pg_type: &Type) -> Type {
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

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Copy, Hash)]
pub struct ColumnId {
    pub table_id: TableId,
    pub column_index: usize,
}

impl ColumnId {
    pub fn get_column<'a>(&self, database: &'a Database) -> &'a PhysicalColumn {
        &database.get_table(self.table_id).columns[self.column_index]
    }

    /// Find the many-to-one relation for the given column. The given column must be a foreign key
    /// column. For example, it could be the `concerts.venue_id` column (assuming [Concert] -> Venue).
    pub fn get_mto_relation(&self, database: &Database) -> Option<ManyToOneId> {
        database
            .relations
            .iter()
            .position(|relation| {
                relation
                    .column_pairs
                    .iter()
                    .any(|pair| &pair.self_column_id == self)
            })
            .map(ManyToOneId)
    }

    /// Find the one-to-many relation for the given column. The given column must be a foreign key
    /// column. For example, it could be the `concerts.venue_id` column (assuming [Concert] -> Venue).
    pub fn get_otm_relation(&self, database: &Database) -> Option<OneToManyId> {
        self.get_mto_relation(database).map(OneToManyId)
    }
}

impl PartialOrd for ColumnId {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ColumnId {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        fn tupled(a: &ColumnId) -> (usize, usize) {
            (a.table_id.arr_idx(), a.column_index)
        }
        tupled(self).cmp(&tupled(other))
    }
}
