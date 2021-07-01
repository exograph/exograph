use id_arena::Arena;
use payas_model::sql::{column::PhysicalColumn, PhysicalTable};

struct SchemaSpec {
    table_specs: Vec<TableSpec>,
}
struct TableSpec {
    name: String,
    column_specs: Vec<ColumnSpec>,
}

struct ColumnSpec {
    name: String,
    db_type: String,
    is_pk: bool,
    foreign_constraint: Option<ForeignConstraint>,
}
struct ForeignConstraint {
    self_column: String,
    foreign_table: String,
}

pub fn schema_stmt(tables: Arena<PhysicalTable>) -> String {
    SchemaSpec::from_tables(tables).stmt()
}

impl SchemaSpec {
    fn stmt(&self) -> String {
        let table_stmts = self
            .table_specs
            .iter()
            .map(|t| (t.stmt()))
            .collect::<Vec<String>>()
            .join("\n\n");

        let foreign_constraint_stmts = self
            .table_specs
            .iter()
            .map(|t| (t.foreign_constraint_stmt()))
            .filter(|s| !s.is_empty())
            .collect::<Vec<String>>();

        format!(
            "{}\n\n\n{}",
            table_stmts,
            foreign_constraint_stmts.join("\n")
        )
    }

    fn from_tables(tables: Arena<PhysicalTable>) -> SchemaSpec {
        let table_specs: Vec<_> = tables
            .iter()
            .map(|table| TableSpec::from_table(table.1))
            .collect();

        SchemaSpec { table_specs }
    }
}

impl TableSpec {
    fn stmt(&self) -> String {
        let columns: Vec<_> = self.column_specs.iter().map(|c| c.stmt()).collect();

        let column_stmts = columns.join(",\n\t");
        format!("CREATE TABLE \"{}\" (\n\t{}\n);", self.name, column_stmts)
    }

    fn foreign_constraint_stmt(&self) -> String {
        self.column_specs
            .iter()
            .flat_map(|c| c.foreign_constraint_stmt())
            .map(|stmt| format!("ALTER TABLE \"{}\" ADD CONSTRAINT {};\n", self.name, stmt))
            .collect()
    }

    fn from_table(table: &PhysicalTable) -> TableSpec {
        let column_specs: Vec<_> = table.columns.iter().map(ColumnSpec::from_column).collect();

        TableSpec {
            name: table.name.clone(),
            column_specs,
        }
    }
}

impl ColumnSpec {
    fn stmt(&self) -> String {
        let pk_str = if self.is_pk { " PRIMARY KEY" } else { "" };

        format!("\"{}\" {}{}", self.name, self.db_type, pk_str)
    }

    fn foreign_constraint_stmt(&self) -> Option<String> {
        self.foreign_constraint.as_ref().map(|c| c.stmt())
    }

    fn from_column(column: &PhysicalColumn) -> ColumnSpec {
        let foreign_constraint = column
            .references
            .as_ref()
            .map(|references| ForeignConstraint {
                self_column: column.column_name.clone(),
                foreign_table: references.table_name.clone(),
            });

        ColumnSpec {
            name: column.column_name.clone(),
            db_type: column.typ.db_type(column.is_autoincrement),
            is_pk: column.is_pk,
            foreign_constraint,
        }
    }
}

impl ForeignConstraint {
    fn stmt(&self) -> String {
        format!(
            "{}_fk FOREIGN KEY ({}) REFERENCES \"{}\"",
            self.foreign_table, self.self_column, self.foreign_table
        )
    }
}
