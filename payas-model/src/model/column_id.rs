use serde::{Deserialize, Serialize};

use crate::sql::{column::PhysicalColumn, PhysicalTable};

use super::{mapped_arena::SerializableSlabIndex, system::ModelSystem};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ColumnId {
    pub table_id: SerializableSlabIndex<PhysicalTable>,
    column_index: usize,
}

impl ColumnId {
    pub fn new(table_id: SerializableSlabIndex<PhysicalTable>, column_index: usize) -> ColumnId {
        ColumnId {
            table_id,
            column_index,
        }
    }

    pub fn get_column<'a>(&self, system: &'a ModelSystem) -> &'a PhysicalColumn {
        &system.tables[self.table_id].columns[self.column_index]
    }
}
