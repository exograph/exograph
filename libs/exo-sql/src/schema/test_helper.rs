#![cfg(test)]

use crate::PhysicalTableName;

use super::column_spec::{ColumnReferenceSpec, ColumnSpec, ColumnTypeSpec};

pub fn pk_column(name: impl Into<String>) -> ColumnSpec {
    ColumnSpec {
        name: name.into(),
        typ: ColumnTypeSpec::Int {
            bits: crate::IntBits::_16,
        },
        is_pk: true,
        is_auto_increment: true,
        is_nullable: false,
        unique_constraints: vec![],
        default_value: None,
        group_name: None,
    }
}

pub fn pk_reference_column(
    name: impl Into<String>,
    foreign_table_name: impl Into<String>,
    foreign_table_schema_name: Option<&str>,
) -> ColumnSpec {
    ColumnSpec {
        name: name.into(),
        typ: ColumnTypeSpec::ColumnReference(ColumnReferenceSpec {
            foreign_table_name: PhysicalTableName::new(
                foreign_table_name,
                foreign_table_schema_name,
            ),
            foreign_pk_column_name: "id".to_string(),
            foreign_pk_type: Box::new(ColumnTypeSpec::Int {
                bits: crate::IntBits::_16,
            }),
        }),
        is_pk: false,
        is_auto_increment: false,
        is_nullable: false,
        unique_constraints: vec![],
        default_value: None,
        group_name: None,
    }
}

pub fn int_column(name: impl Into<String>) -> ColumnSpec {
    ColumnSpec {
        name: name.into(),
        typ: ColumnTypeSpec::Int {
            bits: crate::IntBits::_16,
        },
        is_pk: false,
        is_auto_increment: false,
        is_nullable: false,
        unique_constraints: vec![],
        default_value: None,
        group_name: None,
    }
}

pub fn string_column(name: impl Into<String>) -> ColumnSpec {
    ColumnSpec {
        name: name.into(),
        typ: ColumnTypeSpec::String { max_length: None },
        is_pk: false,
        is_auto_increment: false,
        is_nullable: false,
        unique_constraints: vec![],
        default_value: None,
        group_name: None,
    }
}

pub fn json_column(name: impl Into<String>) -> ColumnSpec {
    ColumnSpec {
        name: name.into(),
        typ: ColumnTypeSpec::Json,
        is_pk: false,
        is_auto_increment: false,
        is_nullable: false,
        unique_constraints: vec![],
        default_value: None,
        group_name: None,
    }
}
