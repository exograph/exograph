use crate::{Database, PhysicalColumn, PhysicalColumnType, PhysicalTable};

use super::{column_spec::ColumnTypeSpec, table_spec::TableSpec};

pub struct DatabaseSpec {
    tables: Vec<TableSpec>,
}

impl DatabaseSpec {
    pub fn new(tables: Vec<TableSpec>) -> Self {
        Self { tables }
    }
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
