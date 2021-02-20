use itertools::Itertools;
use super::{Expression, ParameterBinding, column::{Column, PhysicalColumn}, predicate::Predicate};

#[derive(Debug)]
pub enum Table<'a> {
    Physical(PhysicalTable),
    PredicateTable(&'a Table<'a>, Predicate<'a>),
    Select(&'a Table<'a>, Vec<&'a Column<'a>>),
    CTE(String)
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
            Table::PredicateTable(table, _) => table.get_column(column_name),
            Table::Select(table, _) => table.get_column(column_name),
            Table::CTE(_) => unreachable!() // A flaw in our model?
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
            Table::Select(table, columns) => {
                let table_binding = table.binding();

                let (col_stmtss, col_paramss): (Vec<_>, Vec<_>) = columns.iter().map(|c| {
                    let col_binding = c.binding();
                    (col_binding.stmt, col_binding.params)
                }).unzip();

                let cols_stmts: String = col_stmtss.into_iter().map(|s| s.to_string()).intersperse(String::from(", ")).collect();

                let mut params: Vec<_> = col_paramss.into_iter().flatten().collect();
                params.extend(table_binding.params);
                ParameterBinding::new(format!("select {} from {}", cols_stmts, table_binding.stmt), params)
            }
            Table::CTE(name) => {
                ParameterBinding::new(name.to_owned(), vec![])
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

    #[test]
    fn select_table() {
        let table = Table::Physical(PhysicalTable{
            name: "people".to_string(),
            columns: vec![PhysicalColumn { name: "age".to_string(), table_name: "people".to_string()}]
        });

        let age_col = table.get_column("age").unwrap();
        let age_value_col = Column::Literal(Box::new(5));

        let predicated_table = Table::PredicateTable(&table, Predicate::Eq(&age_col, &age_value_col));

        let select_table = Table::Select(&predicated_table, vec![&age_col]);
        assert_binding!(&select_table.binding(), "select people.age from people where people.age = ?", 5);
    }

    #[test]
    fn cte() {
        let cte = Table::CTE(String::from("my_cte"));
        assert_binding!(&cte.binding(), "my_cte");
    }
}
