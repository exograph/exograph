use crate::{Database, FloatBits, IntBits, PhysicalColumn, PhysicalColumnType, PhysicalTable};

pub struct DatabaseSpec {
    tables: Vec<TableSpec>,
}

impl DatabaseSpec {
    pub fn new(tables: Vec<TableSpec>) -> Self {
        Self { tables }
    }
}

pub struct TableSpec {
    name: String,
    columns: Vec<ColumnSpec>,
}

impl TableSpec {
    pub fn new(name: impl Into<String>, columns: Vec<ColumnSpec>) -> Self {
        Self {
            name: name.into(),
            columns,
        }
    }
}

pub struct ColumnSpec {
    name: String,
    typ: ColumnTypeSpec,
    is_pk: bool,
    is_auto_increment: bool,
    is_nullable: bool,
    unique_constraints: Vec<String>,
    default_value: Option<String>,
}

pub enum ColumnTypeSpec {
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
    Array {
        typ: Box<ColumnTypeSpec>,
    },
    ColumnReference {
        ref_table_name: String,
        ref_column_name: String,
        ref_pk_type: Box<ColumnTypeSpec>,
    },
    Float {
        bits: FloatBits,
    },
    Numeric {
        precision: Option<usize>,
        scale: Option<usize>,
    },
}

impl DatabaseSpec {
    pub fn to_database(self) -> Database {
        let mut database = Database::default();

        // Step 1: Create tables (without columns)
        let tables: Vec<_> = self
            .tables
            .into_iter()
            .map(|table| {
                let table_id = database.insert_table(PhysicalTable {
                    name: table.name,
                    columns: vec![],
                });
                (table_id, table.columns)
            })
            .collect();

        // Step 2: Add columns to tables
        for (table_id, column_specs) in tables.into_iter() {
            let columns = column_specs
                .into_iter()
                .map(|column_spec| PhysicalColumn {
                    table_id,
                    name: column_spec.name,
                    typ: Self::map_typ(column_spec.typ),
                    is_pk: column_spec.is_pk,
                    is_auto_increment: column_spec.is_auto_increment,
                    is_nullable: column_spec.is_nullable,
                    unique_constraints: column_spec.unique_constraints,
                    default_value: column_spec.default_value,
                })
                .collect();

            database.get_table_mut(table_id).columns = columns;
        }

        database
    }

    fn map_typ(typ: ColumnTypeSpec) -> PhysicalColumnType {
        match typ {
            ColumnTypeSpec::Int { bits } => PhysicalColumnType::Int { bits },
            ColumnTypeSpec::String { max_length } => PhysicalColumnType::String { max_length },
            ColumnTypeSpec::Boolean => PhysicalColumnType::Boolean,
            ColumnTypeSpec::Timestamp {
                timezone,
                precision,
            } => PhysicalColumnType::Timestamp {
                timezone,
                precision,
            },
            ColumnTypeSpec::Date => PhysicalColumnType::Date,
            ColumnTypeSpec::Time { precision } => PhysicalColumnType::Time { precision },
            ColumnTypeSpec::Json => PhysicalColumnType::Json,
            ColumnTypeSpec::Blob => PhysicalColumnType::Blob,
            ColumnTypeSpec::Uuid => PhysicalColumnType::Uuid,
            ColumnTypeSpec::Array { typ } => PhysicalColumnType::Array {
                typ: Box::new(Self::map_typ(*typ)),
            },
            ColumnTypeSpec::ColumnReference {
                ref_table_name,
                ref_column_name,
                ref_pk_type,
            } => PhysicalColumnType::ColumnReference {
                ref_table_name,
                ref_column_name,
                ref_pk_type: Box::new(Self::map_typ(*ref_pk_type)),
            },
            ColumnTypeSpec::Float { bits } => PhysicalColumnType::Float { bits },
            ColumnTypeSpec::Numeric { precision, scale } => {
                PhysicalColumnType::Numeric { precision, scale }
            }
        }
    }
}

#[cfg(test)]
pub mod test_helper {
    use super::{ColumnSpec, ColumnTypeSpec};

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
}
