#![cfg(test)]

use super::column_spec::{ColumnSpec, ColumnTypeSpec};

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
    }
}

pub fn pk_reference_column(
    name: impl Into<String>,
    ref_table_name: impl Into<String>,
) -> ColumnSpec {
    ColumnSpec {
        name: name.into(),
        typ: ColumnTypeSpec::ColumnReference {
            ref_table_name: ref_table_name.into(),
            ref_column_name: "id".to_string(),
            ref_pk_type: Box::new(ColumnTypeSpec::Int {
                bits: crate::IntBits::_16,
            }),
        },
        is_pk: true,
        is_auto_increment: false,
        is_nullable: false,
        unique_constraints: vec![],
        default_value: None,
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
    }
}
