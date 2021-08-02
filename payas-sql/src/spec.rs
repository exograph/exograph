use std::collections::HashSet;

use crate::sql::{
    column::{PhysicalColumn, PhysicalColumnType},
    database::Database,
    PhysicalTable,
};
use anyhow::Result;
use id_arena::Arena;

pub struct SQLStatement {
    pub statement: String,
    pub foreign_constraints: Vec<String>,
}

impl ToString for SQLStatement {
    fn to_string(&self) -> String {
        format!(
            "{}\n\n\n{}",
            self.statement,
            self.foreign_constraints.join("\n")
        )
    }
}

pub struct SchemaSpec {
    table_specs: Vec<TableSpec>,
}

impl SchemaSpec {
    pub fn from_model(tables: Arena<PhysicalTable>) -> SchemaSpec {
        let table_specs: Vec<_> = tables
            .iter()
            .map(|(_, table)| TableSpec::from_model(table))
            .collect();

        SchemaSpec { table_specs }
    }

    pub fn from_db(database: &Database) -> Result<SchemaSpec> {
        const QUERY: &str =
            "SELECT table_name FROM information_schema.tables WHERE table_schema = 'public'";

        let table_specs: Vec<TableSpec> = database
            .create_client()?
            .query(QUERY, &[])?
            .iter()
            .map(|r| {
                let name = r.get("table_name");
                TableSpec::from_db(database, name)
            })
            .collect::<Result<_>>()?;

        Ok(SchemaSpec { table_specs })
    }

    pub fn to_model(&self) -> String {
        self.table_specs
            .iter()
            .map(|table_spec| format!("{}\n\n", table_spec.to_model()))
            .collect()
    }

    pub fn to_sql(&self) -> SQLStatement {
        let mut table_stmts = Vec::new();
        let mut foreign_constraints = Vec::new();

        self.table_specs
            .iter()
            .map(|t| t.to_sql())
            .for_each(|mut s| {
                table_stmts.push(s.statement);
                foreign_constraints.append(&mut s.foreign_constraints);
            });

        SQLStatement {
            statement: table_stmts.join("\n"),
            foreign_constraints,
        }
    }
}

struct TableSpec {
    name: String,
    column_specs: Vec<ColumnSpec>,
}

impl TableSpec {
    fn from_model(table: &PhysicalTable) -> TableSpec {
        let column_specs: Vec<_> = table.columns.iter().map(ColumnSpec::from_model).collect();

        TableSpec {
            name: table.name.clone(),
            column_specs,
        }
    }

    fn from_db(database: &Database, table_name: &str) -> Result<TableSpec> {
        const QUERY: &str =
            "SELECT column_name FROM information_schema.columns WHERE table_name = $1";

        let column_specs = database
            .create_client()?
            .query(QUERY, &[&table_name])?
            .iter()
            .map(|r| {
                let name = r.get("column_name");
                ColumnSpec::from_db(database, table_name, name)
            })
            .collect::<Result<Vec<_>>>()?;

        Ok(TableSpec {
            name: table_name.to_string(),
            column_specs,
        })
    }

    fn to_model(&self) -> String {
        let table_annot = format!("@table(\"{}\")", self.name);
        let column_stmts = self
            .column_specs
            .iter()
            .map(|c| format!("  {}\n", c.to_model()))
            .collect::<String>();

        format!(
            "{}\nmodel {} {{\n{}}}",
            table_annot, self.name, column_stmts
        )
    }

    fn to_sql(&self) -> SQLStatement {
        let mut foreign_constraints = Vec::new();
        let column_stmts: String = self
            .column_specs
            .iter()
            .map(|c| c.to_sql(&self.name))
            .map(|mut s| {
                foreign_constraints.append(&mut s.foreign_constraints);
                s.statement
            })
            .collect::<Vec<_>>()
            .join(",\n\t");

        SQLStatement {
            statement: format!("CREATE TABLE \"{}\" (\n\t{}\n);", self.name, column_stmts),
            foreign_constraints,
        }
    }
}

struct ColumnSpec {
    name: String,
    db_type: PhysicalColumnType,
    is_pk: bool,
    is_autoincrement: bool,
}

impl ColumnSpec {
    fn from_model(column: &PhysicalColumn) -> ColumnSpec {
        ColumnSpec {
            name: column.column_name.clone(),
            db_type: column.typ.clone(),
            is_pk: column.is_pk,
            is_autoincrement: column.is_autoincrement,
        }
    }

    fn from_db(database: &Database, table_name: &str, column_name: &str) -> Result<ColumnSpec> {
        let db_type_query = format!(
            "
            SELECT format_type(atttypid, atttypmod), attndims
            FROM pg_attribute
            WHERE attrelid = '{}'::regclass AND attname = '{}'",
            table_name, column_name
        );

        let pk_query = format!(
            "
            SELECT a.attname
            FROM pg_index i
                JOIN pg_attribute a ON a.attrelid = i.indrelid AND a.attnum = ANY(i.indkey)
            WHERE i.indrelid = '{}'::regclass AND i.indisprimary",
            table_name
        );

        let serial_columns_query = "SELECT relname FROM pg_class WHERE relkind = 'S'";

        let mut db_client = database.create_client()?;

        let db_type = db_client
            .query(db_type_query.as_str(), &[])?
            .into_iter()
            .next()
            .map(|row| -> (String, i32) { (row.get("format_type"), row.get("attndims")) })
            .map(|(db_type, dims)| {
                db_type + &"[]".repeat(if dims == 0 { 0 } else { (dims - 1) as usize })
            })
            .map(|db_type| PhysicalColumnType::from_string(&db_type))
            .unwrap();

        let is_pk = db_client
            .query(pk_query.as_str(), &[])?
            .into_iter()
            .next()
            .map(|row| -> String { row.get("attname") })
            .map(|name| name == column_name)
            .unwrap();

        let serial_columns = db_client
            .query(serial_columns_query, &[])?
            .into_iter()
            .map(|row| -> String { row.get("relname") })
            .collect::<HashSet<_>>();

        Ok(ColumnSpec {
            name: column_name.to_string(),
            db_type,
            is_pk,
            is_autoincrement: serial_columns
                .contains(&format!("{}_{}_seq", table_name, column_name)),
        })
    }

    fn to_model(&self) -> String {
        let pk_str = if self.is_pk { " @pk" } else { "" };
        let autoinc_str = if self.is_autoincrement {
            " @autoincrement"
        } else {
            ""
        };

        let (data_type, annots) = self.db_type.to_model();

        format!(
            "{}: {}{}{}",
            self.name,
            data_type + &annots,
            pk_str,
            autoinc_str
        )
    }

    fn to_sql(&self, table_name: &str) -> SQLStatement {
        let SQLStatement {
            statement,
            foreign_constraints,
        } = self.db_type.to_sql(table_name, self.is_autoincrement);
        let pk_str = if self.is_pk { " PRIMARY KEY" } else { "" };

        SQLStatement {
            statement: format!("\"{}\" {}{}", self.name, statement, pk_str),
            foreign_constraints,
        }
    }
}
