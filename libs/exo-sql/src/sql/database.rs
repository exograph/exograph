// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use crate::{ColumnId, PhysicalTable};

use serde::{Deserialize, Serialize};
use typed_generational_arena::{Arena, IgnoreGeneration, Index};

pub type SerializableSlab<T> = Arena<T, usize, IgnoreGeneration>;
pub type TableId = Index<PhysicalTable, usize, IgnoreGeneration>;

#[derive(Serialize, Deserialize, Debug)]
pub struct Database {
    tables: SerializableSlab<PhysicalTable>,
}

impl Database {
    pub fn get_table(&self, id: TableId) -> &PhysicalTable {
        &self.tables[id]
    }

    pub fn get_column_ids(&self, table_id: TableId) -> Vec<ColumnId> {
        (0..self.tables[table_id].columns.len())
            .map(|column_index| ColumnId::new(table_id, column_index))
            .collect()
    }

    pub fn get_table_mut(&mut self, id: TableId) -> &mut PhysicalTable {
        &mut self.tables[id]
    }

    pub fn tables(&self) -> &SerializableSlab<PhysicalTable> {
        &self.tables
    }

    pub fn insert_table(&mut self, table: PhysicalTable) -> TableId {
        self.tables.insert(table)
    }

    // TODO: Make it `pub(crate)`, since we need to resolve table names only during schema building (and not during resolution)
    pub fn get_table_id(&self, table_name: &str) -> Option<TableId> {
        self.tables.iter().find_map(|(id, table)| {
            if table.name == table_name {
                Some(id)
            } else {
                None
            }
        })
    }

    pub(crate) fn get_pk_column(&self, table_id: TableId) -> Option<ColumnId> {
        let table = self.get_table(table_id);
        table
            .get_pk_column_index()
            .map(|column_index| ColumnId::new(table_id, column_index))
    }

    #[cfg(test)]
    pub(crate) fn get_column_id(&self, table_id: TableId, column_name: &str) -> Option<ColumnId> {
        self.tables[table_id]
            .column_index(column_name)
            .map(|column_index| ColumnId::new(table_id, column_index))
    }
}

impl Default for Database {
    fn default() -> Self {
        Database {
            tables: SerializableSlab::new(),
        }
    }
}
