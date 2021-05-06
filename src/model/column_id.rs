use id_arena::Id;

use crate::sql::{column::PhysicalColumn, PhysicalTable};

use super::system::ModelSystem;

#[derive(Debug, Clone)]
pub struct ColumnId {
    pub table_id: Id<PhysicalTable>,
    column_index: usize,
}

impl ColumnId {
    pub fn new(table_id: Id<PhysicalTable>, column_index: usize) -> ColumnId {
        ColumnId {
            table_id,
            column_index,
        }
    }

    pub fn get_column<'a>(&self, system: &'a ModelSystem) -> &'a PhysicalColumn {
        &system.tables[self.table_id].columns[self.column_index]
    }
}
