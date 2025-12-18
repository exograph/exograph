// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::fmt::{Debug, Formatter};

use crate::{ColumnId, ManyToOne, PhysicalColumn, PhysicalTable, SchemaObjectName};

use serde::{Deserialize, Serialize};
use typed_generational_arena::{Arena, IgnoreGeneration, Index};

use super::physical_table::PhysicalEnum;

pub type SerializableSlab<T> = Arena<T, usize, IgnoreGeneration>;
pub type TableId = Index<PhysicalTable, usize, IgnoreGeneration>;
pub type EnumId = Index<PhysicalEnum, usize, IgnoreGeneration>;
#[derive(Serialize, Deserialize)]
pub struct Database {
    tables: SerializableSlab<PhysicalTable>,
    enums: SerializableSlab<PhysicalEnum>,
    pub relations: Vec<ManyToOne>,
}

impl Database {
    pub fn get_table(&self, id: TableId) -> &PhysicalTable {
        &self.tables[id]
    }

    pub fn get_enum(&self, id: EnumId) -> &PhysicalEnum {
        &self.enums[id]
    }

    pub fn get_column_ids(&self, table_id: TableId) -> Vec<ColumnId> {
        (0..self.tables[table_id].columns.len())
            .map(|column_index| new_column_id(table_id, column_index))
            .collect()
    }

    pub fn get_table_mut(&mut self, id: TableId) -> &mut PhysicalTable {
        &mut self.tables[id]
    }

    pub fn get_enum_mut(&mut self, id: EnumId) -> &mut PhysicalEnum {
        &mut self.enums[id]
    }

    pub fn tables(&self) -> &SerializableSlab<PhysicalTable> {
        &self.tables
    }

    pub fn enums(&self) -> &SerializableSlab<PhysicalEnum> {
        &self.enums
    }

    pub fn insert_table(&mut self, table: PhysicalTable) -> TableId {
        self.tables.insert(table)
    }

    pub fn insert_enum(&mut self, enum_: PhysicalEnum) -> EnumId {
        self.enums.insert(enum_)
    }

    pub fn get_table_id(&self, table_name: &SchemaObjectName) -> Option<TableId> {
        self.tables.iter().find_map(|(id, table)| {
            if &table.name == table_name {
                Some(id)
            } else {
                None
            }
        })
    }

    pub fn get_enum_id(&self, enum_name: &SchemaObjectName) -> Option<EnumId> {
        self.enums.iter().find_map(|(id, enum_)| {
            if &enum_.name == enum_name {
                Some(id)
            } else {
                None
            }
        })
    }

    pub fn get_pk_column_ids(&self, table_id: TableId) -> Vec<ColumnId> {
        let table = self.get_table(table_id);
        table
            .get_pk_column_indices()
            .into_iter()
            .map(|column_index| new_column_id(table_id, column_index))
            .collect()
    }

    pub fn get_column_id(&self, table_id: TableId, column_name: &str) -> Option<ColumnId> {
        self.tables[table_id]
            .column_index(column_name)
            .map(|column_index| new_column_id(table_id, column_index))
    }

    pub fn get_column_ids_from_names(
        &self,
        table_id: TableId,
        column_names: &[String],
    ) -> Vec<ColumnId> {
        column_names
            .iter()
            .map(|column_name| {
                self.get_column_id(table_id, column_name)
                    .unwrap_or_else(|| {
                        let table_name = self.tables[table_id].name.fully_qualified_name();
                        panic!(
                            "Column '{}' not found in table '{}'",
                            column_name, table_name
                        )
                    })
            })
            .collect()
    }

    pub fn get_column_mut(&mut self, column_id: ColumnId) -> &mut PhysicalColumn {
        let table = self.get_table_mut(column_id.table_id);
        &mut table.columns[column_id.column_index]
    }
}

fn new_column_id(table_id: TableId, column_index: usize) -> ColumnId {
    ColumnId {
        table_id,
        column_index,
    }
}

impl Default for Database {
    fn default() -> Self {
        Database {
            tables: SerializableSlab::new(),
            enums: SerializableSlab::new(),
            relations: vec![],
        }
    }
}

impl Debug for Database {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        for (id, table) in self.tables.iter() {
            writeln!(f, "{}: {}", id.arr_idx(), table.name.fully_qualified_name())?;
            writeln!(f, "  columns: ")?;
            for (column_id, column) in table.columns.iter().enumerate() {
                writeln!(f, "    {column_id}: {column:?}")?;
            }
        }

        Ok(())
    }
}
