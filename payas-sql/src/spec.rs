use std::collections::{HashMap, HashSet};

use crate::sql::{
    column::{PhysicalColumn, PhysicalColumnType},
    database::Database,
    PhysicalTable,
};
use anyhow::Result;
use id_arena::Arena;
use regex::Regex;

pub struct SQLStatement {
    pub statement: String,
    pub foreign_constraints: Vec<String>,
}

impl ToString for SQLStatement {
    fn to_string(&self) -> String {
        format!(
            "{}\n{}",
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
                table_stmts.push(s.statement + "\n");
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
        let constraints_query = format!(
            "
            SELECT contype, pg_get_constraintdef(oid, true) as condef
            FROM pg_constraint
            WHERE
                conrelid = '{}'::regclass AND conparentid = 0",
            table_name
        );

        let columns_query = format!(
            "SELECT column_name FROM information_schema.columns WHERE table_name = '{}'",
            table_name
        );

        let primary_key_re = Regex::new(r"PRIMARY KEY \(([^)]+)\)").unwrap();
        let foreign_key_re =
            Regex::new(r"FOREIGN KEY \(([^)]+)\) REFERENCES ([^\(]+)\(([^)]+)\)").unwrap();

        let mut db_client = database.create_client()?;

        let constraints = db_client
            .query(constraints_query.as_str(), &[])?
            .iter()
            .map(|row| -> (i8, String) { (row.get("contype"), row.get("condef")) })
            .map(|(contype, condef)| (contype as u8 as char, condef))
            .collect::<Vec<_>>();

        let primary_keys = constraints
            .iter()
            .filter(|(contype, _)| *contype == 'p')
            .map(|(_, condef)| primary_key_re.captures_iter(condef).next().unwrap()[1].to_owned())
            .collect::<HashSet<_>>();

        let mut foreign_constraints = HashMap::new();
        for (_, condef) in constraints.iter().filter(|(contype, _)| *contype == 'f') {
            let matches = foreign_key_re.captures_iter(condef).next().unwrap();
            let column_name = matches[1].to_owned();
            let ref_table_name = matches[2].to_owned();
            let ref_column_name = matches[3].to_owned();

            foreign_constraints.insert(
                column_name.clone(),
                PhysicalColumnType::ColumnReference {
                    column_name: column_name.clone(),
                    ref_table_name: ref_table_name.clone(),
                    ref_pk_type: Box::new(
                        ColumnSpec::from_db(
                            database,
                            &ref_table_name,
                            &ref_column_name,
                            true,
                            None,
                        )?
                        .db_type,
                    ),
                },
            );
        }

        let column_specs = db_client
            .query(columns_query.as_str(), &[])?
            .iter()
            .map(|r| {
                let name = r.get("column_name");
                ColumnSpec::from_db(
                    database,
                    table_name,
                    name,
                    primary_keys.contains(name),
                    foreign_constraints.get(name).cloned(),
                )
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

    fn from_db(
        database: &Database,
        table_name: &str,
        column_name: &str,
        is_pk: bool,
        explicit_type: Option<PhysicalColumnType>,
    ) -> Result<ColumnSpec> {
        let serial_columns_query = "SELECT relname FROM pg_class WHERE relkind = 'S'";

        let mut db_client = database.create_client()?;

        let db_type = explicit_type.unwrap_or({
            let db_type_query = format!(
                "
                    SELECT format_type(atttypid, atttypmod), attndims
                    FROM pg_attribute
                    WHERE attrelid = '{}'::regclass AND attname = '{}'",
                table_name, column_name
            );

            db_client
                .query(db_type_query.as_str(), &[])?
                .get(0)
                .map(|row| -> (String, i32) { (row.get("format_type"), row.get("attndims")) })
                .map(|(db_type, dims)| {
                    db_type + &"[]".repeat(if dims == 0 { 0 } else { (dims - 1) as usize })
                })
                .map(|db_type| PhysicalColumnType::from_string(&db_type))
                .unwrap()
        });

        let serial_columns = db_client
            .query(serial_columns_query, &[])?
            .iter()
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
