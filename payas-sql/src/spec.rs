use crate::sql::{column::PhysicalColumn, PhysicalTable};
use id_arena::Arena;

pub type SQLStatement = String;

pub struct SchemaSpec {
    table_specs: Vec<TableSpec>,
}

impl SchemaSpec {
    pub fn from_tables(tables: Arena<PhysicalTable>) -> SchemaSpec {
        let table_specs: Vec<_> = tables
            .iter()
            .map(|(_, table)| TableSpec::from_table(table))
            .collect();

        SchemaSpec { table_specs }
    }

    pub fn to_sql(&self) -> SQLStatement {
        let table_stmts = self
            .table_specs
            .iter()
            .map(|t| t.to_sql())
            .collect::<Vec<_>>()
            .join("\n\n");

        let foreign_constraint_stmts = self
            .table_specs
            .iter()
            .map(|t| t.foreign_constraints_sql())
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("\n");

        format!("{}\n\n\n{}", table_stmts, foreign_constraint_stmts)
    }
}

struct TableSpec {
    name: String,
    column_specs: Vec<ColumnSpec>,
}

impl TableSpec {
    fn from_table(table: &PhysicalTable) -> TableSpec {
        let column_specs: Vec<_> = table.columns.iter().map(ColumnSpec::from_column).collect();

        TableSpec {
            name: table.name.clone(),
            column_specs,
        }
    }

    fn to_sql(&self) -> SQLStatement {
        let column_stmts: String = self
            .column_specs
            .iter()
            .map(|c| c.to_sql())
            .collect::<Vec<_>>()
            .join(",\n\t");

        format!("CREATE TABLE \"{}\" (\n\t{}\n);", self.name, column_stmts)
    }

    fn foreign_constraints_sql(&self) -> SQLStatement {
        self.column_specs
            .iter()
            .flat_map(|c| c.foreign_constraint_sql())
            .map(|stmt| format!("ALTER TABLE \"{}\" ADD CONSTRAINT {};\n", self.name, stmt))
            .collect()
    }
}

struct ColumnSpec {
    name: String,
    db_type: String,
    is_pk: bool,
    foreign_constraint: Option<(String, String)>, // column, foreign table
}

impl ColumnSpec {
    fn from_column(column: &PhysicalColumn) -> ColumnSpec {
        let foreign_constraint = column
            .references
            .as_ref()
            .map(|references| (column.column_name.clone(), references.table_name.clone()));

        ColumnSpec {
            name: column.column_name.clone(),
            db_type: column.typ.db_type(column.is_autoincrement),
            is_pk: column.is_pk,
            foreign_constraint,
        }
    }

    fn to_sql(&self) -> SQLStatement {
        let pk_str = if self.is_pk { " PRIMARY KEY" } else { "" };

        format!("\"{}\" {}{}", self.name, self.db_type, pk_str)
    }

    fn foreign_constraint_sql(&self) -> Option<SQLStatement> {
        self.foreign_constraint
            .as_ref()
            .map(|(column, foreign_table)| {
                format!(
                    "{table}_fk FOREIGN KEY ({column}) REFERENCES \"{table}\"",
                    table = foreign_table,
                    column = column
                )
            })
    }
}
