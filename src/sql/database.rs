use super::{column::PhysicalColumn, table::PhysicalTable};
use std::sync::Arc;

pub struct Database {
    pub tables: Vec<Arc<PhysicalTable>>,
}

impl Database {
    pub fn empty() -> Self {
        Self { tables: vec![] }
    }

    pub fn get_table(&self, table_name: &str) -> Option<Arc<PhysicalTable>> {
        self
            .tables
            .iter()
            .find(|table| table.name.as_str() == table_name).map(|t| t.clone())
    }

    pub fn get_or_create_table(
        &mut self,
        table_name: &str,
        column_names: &[&str],
    ) -> Arc<PhysicalTable> {
        match self.get_table(table_name)
        {
            Some(table) => table,
            None => {
                let table = Arc::new(PhysicalTable {
                    name: table_name.to_string(),
                    columns: column_names
                        .iter()
                        .map(|column_name| {
                            Arc::new(PhysicalColumn {
                                name: column_name.to_string(),
                                table_name: table_name.to_string(),
                            })
                        })
                        .collect(),
                });
                self.tables.push(table.clone());
                table
            }
        }
    }
}
