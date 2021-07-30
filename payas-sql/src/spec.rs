use std::collections::HashSet;

use crate::sql::{
    column::{PhysicalColumn, PhysicalColumnType},
    database::Database,
    PhysicalTable,
};
use anyhow::Result;
use id_arena::Arena;

pub type ModelStatement = String;
pub type SQLStatement = String;

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

    pub fn to_model(&self) -> ModelStatement {
        self.table_specs
            .iter()
            .map(|table_spec| format!("{}\n\n", table_spec.to_model()))
            .collect()
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

    fn to_model(&self) -> ModelStatement {
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
    db_type: PhysicalColumnType,
    is_pk: bool,
    is_autoincrement: bool,
    foreign_constraint: Option<(String, String)>, // column, foreign table
}

impl ColumnSpec {
    fn from_model(column: &PhysicalColumn) -> ColumnSpec {
        let foreign_constraint = column
            .references
            .as_ref()
            .map(|references| (column.column_name.clone(), references.table_name.clone()));

        ColumnSpec {
            name: column.column_name.clone(),
            db_type: column.typ.clone(),
            is_pk: column.is_pk,
            is_autoincrement: column.is_autoincrement,
            foreign_constraint,
        }
    }

    fn from_db(database: &Database, table_name: &str, column_name: &str) -> Result<ColumnSpec> {
        let query = format!(
            "SELECT data_type, datetime_precision FROM information_schema.columns WHERE table_name = '{}' AND column_name = '{}'",
             table_name, column_name
        );

        let pk_query = format!("
            SELECT pg_attribute.attname
            FROM
                pg_class
                JOIN pg_index ON pg_class.oid = pg_index.indrelid AND pg_index.indisprimary
                JOIN pg_attribute ON pg_class.oid = pg_attribute.attrelid AND pg_attribute.attnum = ANY(pg_index.indkey)
            WHERE pg_class.oid = '{}'::regclass",
            table_name
        );

        let mut db_client = database.create_client()?;
        let row = db_client
            .query(query.as_str(), &[])?
            .into_iter()
            .next()
            .unwrap();
        let primary_keys = db_client
            .query(pk_query.as_str(), &[])?
            .into_iter()
            .map(|row| -> String { row.get("attname") })
            .collect::<HashSet<_>>();

        let data_type = {
            let r: String = row.get("data_type");
            r.to_uppercase()
        };

        let datetime_precision = {
            let r: Option<i32> = row.get("datetime_precision");
            r.map(|p| p as usize)
        };

        let mut db_type = PhysicalColumnType::from_string(&data_type);
        match &mut db_type {
            PhysicalColumnType::Timestamp {
                timezone: _,
                precision,
            } => {
                *precision = datetime_precision;
            }
            PhysicalColumnType::Time { precision } => {
                *precision = datetime_precision;
            }
            _ => (),
        }

        Ok(ColumnSpec {
            name: column_name.to_string(),
            db_type,
            is_pk: primary_keys.contains(column_name),
            is_autoincrement: false,  // TODO
            foreign_constraint: None, // TODO
        })
    }

    fn to_model(&self) -> ModelStatement {
        let pk_str = if self.is_pk { " @pk" } else { "" };
        let autoinc_str = if self.is_autoincrement {
            " @autoincrement"
        } else {
            ""
        };

        format!(
            "{}: {}{}{}",
            self.name,
            self.db_type.to_model(),
            pk_str,
            autoinc_str
        )
    }

    fn to_sql(&self) -> SQLStatement {
        let pk_str = if self.is_pk { " PRIMARY KEY" } else { "" };

        format!(
            "\"{}\" {}{}",
            self.name,
            self.db_type.to_sql(self.is_autoincrement),
            pk_str
        )
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
