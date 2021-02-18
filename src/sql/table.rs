use super::{Expression, ParameterBinding, column::{Column, PhysicalColumn}, predicate::Predicate};

#[derive(Debug)]
pub enum Table<'a> {
    Physical(PhysicalTable),
    PredicateTable(&'a Table<'a>, Predicate<'a>)
}

#[derive(Debug)]
pub struct PhysicalTable {
    pub name: String,
    pub columns: Vec<PhysicalColumn>
}

impl PhysicalTable {
    pub fn get_column<'a>(&'a self, column_name: &str) -> Option<&'a PhysicalColumn> {
        self.columns.iter().find(|column| column.name.as_str() == column_name)
    }
}

impl Table<'_> {
    pub fn get_column(&self, column_name: &str) -> Option<Column> {
        match self {
            Table::Physical(physical_table) => physical_table.get_column(column_name).map(|c| Column::Physical(c)),
            Table::PredicateTable(table, _) => table.get_column(column_name)
        }
    }
}

impl Expression for PhysicalTable {
    fn binding(&self) -> ParameterBinding {
        ParameterBinding::new(self.name.clone(), vec![])
    }
}

impl<'a> Expression for Table<'a> {
    fn binding(&self) -> ParameterBinding {
        match self {
            Table::Physical(physical_table) => physical_table.binding(),
            Table::PredicateTable(table, predicate) => {
                let table_binding = table.binding();
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
    fn phyrical_table() {
        let table = Table::Physical(PhysicalTable{
            name: "people".to_string(),
            columns: vec![]
        });
        assert_binding!(&table.binding(), "people");
    }

    #[test]
    fn predicated_table() {
        let table = Table::Physical(PhysicalTable{
            name: "people".to_string(),
            columns: vec![PhysicalColumn { name: "age".to_string(), table_name: "people".to_string()}]
        });

        let age_col = table.get_column("age").unwrap();
        let age_value_col = Column::Literal(Box::new(5));

        let predicated_table = Table::PredicateTable(&table, Predicate::Eq(&age_col, &age_value_col));
        assert_binding!(&predicated_table.binding(), "people where people.age = ?", 5);
    }
}
