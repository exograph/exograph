use super::{column::PhysicalColumn, table::PhysicalTable};

pub struct Database {
    pub tables: Vec<PhysicalTable>,
}

impl Database {
    pub fn empty() -> Self {
        Self { tables: vec![] }
    }

    pub fn get_table(&self, table_name: &str) -> Option<&PhysicalTable> {
        self
            .tables
            .iter()
            .find(|table| table.name == table_name)
    }

    pub fn create_table(
        &mut self,
        table_name: &str,
        column_names: &[&str],
    ) {
        match self.get_table(table_name)
        {
            Some(_) => (),
            None => {
                let table = PhysicalTable {
                    name: table_name.to_string(),
                    columns: column_names
                        .iter()
                        .map(|column_name| {
                            PhysicalColumn {
                                name: column_name.to_string(),
                                table_name: table_name.to_string(),
                            }
                        })
                        .collect(),
                };
                self.tables.push(table);
            }
        }
    }
}
