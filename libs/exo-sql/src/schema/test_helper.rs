#![cfg(test)]

use crate::{PhysicalColumnType, SchemaObjectName};

use super::column_spec::{
    ColumnAutoincrement, ColumnDefault, ColumnReferenceSpec, ColumnSpec, ColumnTypeSpec,
};

pub fn pk_column(name: impl Into<String>) -> ColumnSpec {
    ColumnSpec {
        name: name.into(),
        typ: ColumnTypeSpec::Direct(PhysicalColumnType::Int {
            bits: crate::IntBits::_16,
        }),
        is_pk: true,
        is_nullable: false,
        unique_constraints: vec![],
        default_value: Some(ColumnDefault::Autoincrement(ColumnAutoincrement::Serial)),
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
        typ: ColumnTypeSpec::Reference(ColumnReferenceSpec {
            foreign_table_name: SchemaObjectName::new(
                foreign_table_name,
                foreign_table_schema_name,
            ),
            foreign_pk_column_name: "id".to_string(),
            foreign_pk_type: Box::new(PhysicalColumnType::Int {
                bits: crate::IntBits::_16,
            }),
        }),
        is_pk: false,
        is_nullable: false,
        unique_constraints: vec![],
        default_value: None,
        group_name: None,
    }
}

pub fn int_column(name: impl Into<String>) -> ColumnSpec {
    ColumnSpec {
        name: name.into(),
        typ: ColumnTypeSpec::Direct(PhysicalColumnType::Int {
            bits: crate::IntBits::_16,
        }),
        is_pk: false,
        is_nullable: false,
        unique_constraints: vec![],
        default_value: None,
        group_name: None,
    }
}

pub fn string_column(name: impl Into<String>) -> ColumnSpec {
    ColumnSpec {
        name: name.into(),
        typ: ColumnTypeSpec::Direct(PhysicalColumnType::String { max_length: None }),
        is_pk: false,
        is_nullable: false,
        unique_constraints: vec![],
        default_value: None,
        group_name: None,
    }
}

pub fn json_column(name: impl Into<String>) -> ColumnSpec {
    ColumnSpec {
        name: name.into(),
        typ: ColumnTypeSpec::Direct(PhysicalColumnType::Json),
        is_pk: false,
        is_nullable: false,
        unique_constraints: vec![],
        default_value: None,
        group_name: None,
    }
}
