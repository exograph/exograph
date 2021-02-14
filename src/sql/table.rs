use super::{Expression, ParameterBinding, column::{Column, PhysicalColumn}};
use std::sync::Arc;

#[derive(Debug)]
pub enum Table {
    Physical(PhysicalTable),
}

#[derive(Debug)]
pub struct PhysicalTable {
    pub name: String,
    pub columns: Vec<Arc<PhysicalColumn>>
}

impl PhysicalTable {
    pub fn get_column(self: &Arc<Self>, column_name: &str) -> Option<Arc<PhysicalColumn>> {
        self.columns.iter().find(|column| column.name.as_str() == column_name).map(|c| c.clone())
    }
}

impl Table {
    fn column(self: Arc<Table>, name: String) -> Column {
        match self.as_ref() {
            Table::Physical(physical_table) => {
                Column::Physical(physical_table.columns.iter().find(|c| c.name == name).unwrap().clone())
            },
        }
    }
}

impl Expression for PhysicalTable {
    fn binding(&self) -> ParameterBinding {
        ParameterBinding::new(self.name.clone(), vec![])
    }
}

impl Expression for Table {
    fn binding(&self) -> ParameterBinding {
        match self {
            Table::Physical(physical_table) => physical_table.binding(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic() {
        let table = Table::Physical(PhysicalTable{
            name: "people".to_string(),
            columns: vec![]
        });
        assert_binding!(&table.binding(), "people");
    }
}
