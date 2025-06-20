#![cfg(test)]

use crate::SchemaObjectName;
use crate::sql::physical_column_type::{IntBits, IntColumnType, JsonColumnType, StringColumnType};

use super::column_spec::{ColumnAutoincrement, ColumnDefault, ColumnReferenceSpec, ColumnSpec};

pub fn pk_column(name: impl Into<String>) -> ColumnSpec {
    ColumnSpec {
        name: name.into(),
        typ: Box::new(IntColumnType { bits: IntBits::_16 }),
        reference_spec: None,
        is_pk: true,
        is_nullable: false,
        unique_constraints: vec![],
        default_value: Some(ColumnDefault::Autoincrement(ColumnAutoincrement::Serial)),
        group_names: vec![],
    }
}

pub fn pk_reference_column(
    name: impl Into<String>,
    foreign_table_name: impl Into<String>,
    foreign_table_schema_name: Option<&str>,
) -> ColumnSpec {
    ColumnSpec {
        name: name.into(),
        typ: Box::new(IntColumnType { bits: IntBits::_16 }),
        reference_spec: Some(ColumnReferenceSpec {
            foreign_table_name: SchemaObjectName::new(
                foreign_table_name,
                foreign_table_schema_name,
            ),
            foreign_pk_column_name: "id".to_string(),
            foreign_pk_type: Box::new(IntColumnType { bits: IntBits::_16 }),
        }),
        is_pk: false,
        is_nullable: false,
        unique_constraints: vec![],
        default_value: None,
        group_names: vec![],
    }
}

pub fn int_column(name: impl Into<String>) -> ColumnSpec {
    ColumnSpec {
        name: name.into(),
        typ: Box::new(IntColumnType { bits: IntBits::_16 }),
        reference_spec: None,
        is_pk: false,
        is_nullable: false,
        unique_constraints: vec![],
        default_value: None,
        group_names: vec![],
    }
}

pub fn string_column(name: impl Into<String>) -> ColumnSpec {
    ColumnSpec {
        name: name.into(),
        typ: Box::new(StringColumnType { max_length: None }),
        reference_spec: None,
        is_pk: false,
        is_nullable: false,
        unique_constraints: vec![],
        default_value: None,
        group_names: vec![],
    }
}

pub fn json_column(name: impl Into<String>) -> ColumnSpec {
    ColumnSpec {
        name: name.into(),
        typ: Box::new(JsonColumnType),
        reference_spec: None,
        is_pk: false,
        is_nullable: false,
        unique_constraints: vec![],
        default_value: None,
        group_names: vec![],
    }
}
