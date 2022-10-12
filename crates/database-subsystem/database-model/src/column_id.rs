use payas_sql::PhysicalTable;
use serde::{Deserialize, Serialize};

use payas_sql::PhysicalColumn;

use core_model::mapped_arena::SerializableSlabIndex;

use crate::model::ModelDatabaseSystem;

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
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

    pub fn get_column<'a>(&self, system: &'a ModelDatabaseSystem) -> &'a PhysicalColumn {
        &system.tables[self.table_id].columns[self.column_index]
    }
}
