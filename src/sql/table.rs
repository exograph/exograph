use super::{Expression, ParameterBinding, column::{Column, PhysicalColumn}, predicate::Predicate};
use std::sync::Arc;

#[derive(Debug)]
pub enum Table {
    Physical(PhysicalTable),
    PredicateTable(Box<Table>, Predicate)
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
    fn column(&self, name: String) -> Column {
        match self {
            Table::Physical(physical_table) => {
                Column::Physical(physical_table.columns.iter().find(|c| c.name == name).unwrap().clone())
            },
            Table::PredicateTable(table, _) => {
                table.column(name)
            }
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
            Table::PredicateTable(table, predicate) => {
                let table_binding = table.as_ref().binding();
                let predicate_binding = predicate.binding();
                let stmt = format!("{} where {}", table_binding.stmt, predicate_binding.stmt);
                let mut params = table_binding.params;
                params.extend(predicate_binding.params);

                ParameterBinding::new(stmt, params)
            }
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
