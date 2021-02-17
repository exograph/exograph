use std::sync::Arc;
use super::{Expression, ParameterBinding, SQLParam};

#[derive(Debug)]
pub enum Column {
    Physical(Arc<PhysicalColumn>),
    Plain { name: String },
    Star,
    JsonAgg { column: Box<Column> },
    JsonObj { columns: Vec<ColumnAttr> },
    Literal(Arc<dyn SQLParam>),
    //SingleSelect { column: Box<Column>, table: Table}
}

#[derive(Debug)]
pub struct PhysicalColumn {
    pub name: String,
    pub table_name: String
}

#[derive(Debug)]
pub struct ColumnAttr {
    alias: String,
    column: Column
}

impl Expression for PhysicalColumn {
    fn binding(&self) -> ParameterBinding {
        ParameterBinding::new(format!("{}.{}", self.table_name, self.name), vec![])
    }
}

impl Expression for Column {
    fn binding(&self) -> ParameterBinding {
        match self {
            Column::Physical(physical_column) => {
                physical_column.binding()
            },
            Column::Plain { name } => {
                ParameterBinding::new(name.to_owned(), vec![])
            },
            Column::Star => {
                ParameterBinding::new("*".to_string(), vec![])
            }
            Column::JsonAgg { column } => {
                let col_stmt = column.binding();
                ParameterBinding::new(format!("json_agg({})", col_stmt.stmt), col_stmt.params)
            }
            Column::JsonObj { columns} => {
                let (strs, paramss): (Vec<_>, Vec<_>) = columns.iter().map(|aliased_column| {
                    let col_stmt = aliased_column.column.binding();
                    (format!("'{}', {}", aliased_column.alias, col_stmt.stmt), col_stmt.params)
                }).unzip();

                ParameterBinding::new(strs.join(", "), paramss.into_iter().flatten().collect())
            },
            Column::Literal(value) => {
                ParameterBinding::new("?".to_string(), vec![value.clone()])
            }
        }
    }
}