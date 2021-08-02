//! Conversions between claytip models and SQL tables.
//!
//! Used by `model import` and `schema create` commands.

use std::collections::{HashMap, HashSet};

use crate::sql::{
    column::{PhysicalColumn, PhysicalColumnType},
    database::Database,
    PhysicalTable,
};
use anyhow::Result;
use id_arena::Arena;
use regex::Regex;

/// An SQL statement along with any foreign constraint statements that should follow after all the
/// statements have been executed.
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

/// Specification for the overall schema.
///
/// Represented by a claytip file or an SQL database.
pub struct SchemaSpec {
    table_specs: Vec<TableSpec>,
}

impl SchemaSpec {
    /// Creates a new schema specification from the tables of a claytip model file.
    pub fn from_model(tables: Arena<PhysicalTable>) -> SchemaSpec {
        let table_specs: Vec<_> = tables
            .iter()
            .map(|(_, table)| TableSpec::from_model(table))
            .collect();

        SchemaSpec { table_specs }
    }

    /// Creates a new schema specification from an SQL database.
    pub fn from_db(database: &Database) -> Result<SchemaSpec> {
        // Query to get a list of all the tables in the database
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

    /// Converts the schema specification to a claytip file.
    pub fn to_model(&self) -> String {
        self.table_specs
            .iter()
            .map(|table_spec| format!("{}\n\n", table_spec.to_model()))
            .collect()
    }

    /// Converts the schema specification to SQL statements.
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

/// Specification for a single table.
///
/// Represented by a claytip model or an SQL table.
struct TableSpec {
    name: String,
    column_specs: Vec<ColumnSpec>,
}

impl TableSpec {
    /// Creates a new table specification from a claytip model.
    fn from_model(table: &PhysicalTable) -> TableSpec {
        let column_specs: Vec<_> = table.columns.iter().map(ColumnSpec::from_model).collect();

        TableSpec {
            name: table.name.clone(),
            column_specs,
        }
    }

    /// Creates a new table specification from an SQL table.
    fn from_db(database: &Database, table_name: &str) -> Result<TableSpec> {
        // Query to get a list of constraints in the table (primary key and foreign key constraints)
        let constraints_query = format!(
            "
            SELECT contype, pg_get_constraintdef(oid, true) as condef
            FROM pg_constraint
            WHERE
                conrelid = '{}'::regclass AND conparentid = 0",
            table_name
        );

        // Query to get a list of columns in the table
        let columns_query = format!(
            "SELECT column_name FROM information_schema.columns WHERE table_name = '{}'",
            table_name
        );

        let primary_key_re = Regex::new(r"PRIMARY KEY \(([^)]+)\)").unwrap();
        let foreign_key_re =
            Regex::new(r"FOREIGN KEY \(([^)]+)\) REFERENCES ([^\(]+)\(([^)]+)\)").unwrap();

        let mut db_client = database.create_client()?;

        // Get all the constraints in the table
        let constraints = db_client
            .query(constraints_query.as_str(), &[])?
            .iter()
            .map(|row| {
                let contype: i8 = row.get("contype");
                let condef: String = row.get("condef");

                (contype as u8 as char, condef)
            })
            .collect::<Vec<_>>();

        // Filter out primary key constraints to find which columns are primary keys
        let primary_keys = constraints
            .iter()
            .filter(|(contype, _)| *contype == 'p')
            .map(|(_, condef)| primary_key_re.captures_iter(condef).next().unwrap()[1].to_owned())
            .collect::<HashSet<_>>();

        // Filter out foreign key constraints to find which columns require foreign key constraints
        let mut foreign_constraints = HashMap::new();
        for (_, condef) in constraints.iter().filter(|(contype, _)| *contype == 'f') {
            let matches = foreign_key_re.captures_iter(condef).next().unwrap();
            let column_name = matches[1].to_owned(); // name of the column
            let ref_table_name = matches[2].to_owned(); // name of the table the column refers to
            let ref_column_name = matches[3].to_owned(); // name of the column in the referenced table

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

    /// Converts the table specification to a claytip model.
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

    /// Converts the table specification to SQL statements.
    fn to_sql(&self) -> SQLStatement {
        let mut foreign_constraints = Vec::new();
        let column_stmts: String = self
            .column_specs
            .iter()
            .map(|c| {
                let mut s = c.to_sql(&self.name);
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

/// Specification for a single column.
///
/// Represented by a claytip model field or an SQL column.
struct ColumnSpec {
    name: String,
    db_type: PhysicalColumnType,
    is_pk: bool,
    is_autoincrement: bool,
}

impl ColumnSpec {
    /// Creates a new column specification from a claytip model field.
    fn from_model(column: &PhysicalColumn) -> ColumnSpec {
        ColumnSpec {
            name: column.column_name.clone(),
            db_type: column.typ.clone(),
            is_pk: column.is_pk,
            is_autoincrement: column.is_autoincrement,
        }
    }

    /// Creates a new column specification from an SQL column.
    ///
    /// If the column references another claytip model, the column's type in the claytip file will
    /// be the model's name. The referenced model's name can be specified with `explicit_type`.
    fn from_db(
        database: &Database,
        table_name: &str,
        column_name: &str,
        is_pk: bool,
        explicit_type: Option<PhysicalColumnType>,
    ) -> Result<ColumnSpec> {
        // Find all sequences in the database that are used for SERIAL (autoincrement) columns
        // e.g. an autoincrement column `id` in the table `users` will create a sequence called
        // `users_id_seq`
        let serial_columns_query = "SELECT relname FROM pg_class WHERE relkind = 'S'";

        let mut db_client = database.create_client()?;

        let db_type = explicit_type.unwrap_or({
            // Query to find the type of the column and the # of dimensions if the type is an array
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
                .map(|row| {
                    let mut db_type: String = row.get("format_type");
                    let dims: i32 = row.get("attndims");

                    // When querying array types, the number of dimensions is not correctly shown
                    // e.g. a column declared as `INT[][][]` will be shown as `INT[]`
                    // So we manually query how many dimensions the column has and append `[]` to
                    // the type
                    db_type += &"[]".repeat(if dims == 0 { 0 } else { (dims - 1) as usize });
                    PhysicalColumnType::from_string(&db_type)
                })
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

    /// Converts the column specification to a claytip model.
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

    /// Converts the column specification to SQL statements.
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
