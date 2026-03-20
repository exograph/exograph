// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

//! Test helper to construct a `Database` directly using only core types.
//! Mirrors what `DatabaseSpec::to_database()` does in pg-schema.

use crate::physical_column_type::{
    IntBits, IntColumnType, JsonColumnType, PhysicalColumnType, StringColumnType,
};
use exo_sql_core::{
    ColumnReference, Database, ManyToOne, PhysicalColumn, PhysicalTable, RelationColumnPair,
    SchemaObjectName,
    column_default::{ColumnAutoincrement, ColumnDefault},
};

pub struct TestColumn {
    name: String,
    typ: Box<dyn PhysicalColumnType>,
    is_pk: bool,
    foreign_ref: Option<ForeignRef>,
}

struct ForeignRef {
    table: String,
    column: String,
    group: String,
}

pub fn pk(name: &str) -> TestColumn {
    TestColumn {
        name: name.to_string(),
        typ: Box::new(IntColumnType { bits: IntBits::_16 }),
        is_pk: true,
        foreign_ref: None,
    }
}

pub fn string(name: &str) -> TestColumn {
    TestColumn {
        name: name.to_string(),
        typ: Box::new(StringColumnType { max_length: None }),
        is_pk: false,
        foreign_ref: None,
    }
}

pub fn int(name: &str) -> TestColumn {
    TestColumn {
        name: name.to_string(),
        typ: Box::new(IntColumnType { bits: IntBits::_16 }),
        is_pk: false,
        foreign_ref: None,
    }
}

pub fn json(name: &str) -> TestColumn {
    TestColumn {
        name: name.to_string(),
        typ: Box::new(JsonColumnType),
        is_pk: false,
        foreign_ref: None,
    }
}

pub fn fk(name: &str, table: &str, col: &str, group: &str) -> TestColumn {
    TestColumn {
        name: name.to_string(),
        typ: Box::new(IntColumnType { bits: IntBits::_16 }),
        is_pk: false,
        foreign_ref: Some(ForeignRef {
            table: table.to_string(),
            column: col.to_string(),
            group: group.to_string(),
        }),
    }
}

pub struct DatabaseBuilder {
    tables: Vec<(String, Vec<TestColumn>)>,
}

impl Default for DatabaseBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl DatabaseBuilder {
    pub fn new() -> Self {
        Self { tables: vec![] }
    }

    pub fn table(mut self, name: &str, columns: Vec<TestColumn>) -> Self {
        self.tables.push((name.to_string(), columns));
        self
    }

    pub fn build(self) -> Database {
        let mut database = Database::default();

        // Step 1: Create tables (without columns) to get TableIds
        let table_data: Vec<_> = self
            .tables
            .into_iter()
            .map(|(name, columns)| {
                let table_id = database.insert_table(PhysicalTable {
                    name: SchemaObjectName::new(&name, None),
                    columns: vec![],
                    indices: vec![],
                    managed: true,
                });
                (table_id, name, columns)
            })
            .collect();

        // Step 2: Add columns to tables
        for (table_id, _, columns) in &table_data {
            let physical_columns: Vec<PhysicalColumn> = columns
                .iter()
                .map(|col| PhysicalColumn {
                    table_id: *table_id,
                    name: col.name.clone(),
                    typ: col.typ.clone(),
                    is_pk: col.is_pk,
                    is_nullable: false,
                    unique_constraints: vec![],
                    default_value: if col.is_pk {
                        Some(ColumnDefault::Autoincrement(ColumnAutoincrement::Serial))
                    } else {
                        None
                    },
                    update_sync: false,
                    column_references: None,
                })
                .collect();

            let table = database.get_table_mut(*table_id);
            table.columns = physical_columns;
        }

        // Step 3: Wire up column_references
        for (table_id, _, columns) in &table_data {
            for col in columns {
                if let Some(ref fk_ref) = col.foreign_ref {
                    let foreign_table_id = database
                        .get_table_id(&SchemaObjectName::new(&fk_ref.table, None))
                        .unwrap();
                    let foreign_column_id = database
                        .get_column_id(foreign_table_id, &fk_ref.column)
                        .unwrap();

                    let table = database.get_table_mut(*table_id);
                    let column_index = table
                        .columns
                        .iter()
                        .position(|c| c.name == col.name)
                        .unwrap();
                    table.columns[column_index].column_references = Some(vec![ColumnReference {
                        foreign_column_id,
                        group_name: fk_ref.group.clone(),
                    }]);
                }
            }
        }

        // Step 4: Build ManyToOne relations (same algorithm as DatabaseSpec::to_database)
        let mut relations: Vec<ManyToOne> = Vec::new();

        for (table_id, _, columns) in &table_data {
            let column_ids = database.get_column_ids(*table_id);

            for self_column_id in column_ids {
                let column_name = database.get_table(*table_id).columns
                    [self_column_id.column_index]
                    .name
                    .clone();
                let test_col = columns.iter().find(|c| c.name == column_name).unwrap();

                if let Some(ref fk_ref) = test_col.foreign_ref {
                    let foreign_table_id = database
                        .get_table_id(&SchemaObjectName::new(&fk_ref.table, None))
                        .unwrap();
                    let foreign_pk_column_id = database
                        .get_column_id(foreign_table_id, &fk_ref.column)
                        .unwrap();

                    // Compute alias: same logic as DatabaseSpec::to_database
                    let foreign_table_alias = Some(if column_name.ends_with("_id") {
                        let base_name = &column_name[..column_name.len() - 3];
                        let plural_suffix = if base_name.ends_with('s') { "es" } else { "s" };
                        format!("{base_name}{plural_suffix}")
                    } else {
                        column_name.clone()
                    });

                    relations.push(ManyToOne::new(
                        vec![RelationColumnPair {
                            self_column_id,
                            foreign_column_id: foreign_pk_column_id,
                        }],
                        foreign_table_alias,
                    ));
                }
            }
        }

        database.relations = relations;
        database
    }
}
