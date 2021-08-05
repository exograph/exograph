//! Conversions between claytip models and SQL tables.
//!
//! Used by `model import` and `schema create` commands.

use std::collections::{HashMap, HashSet};
use std::fmt::{Display, Formatter};

use crate::sql::{
    column::{PhysicalColumn, PhysicalColumnType},
    database::Database,
    PhysicalTable,
};
use anyhow::{anyhow, Result};
use heck::CamelCase;
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

pub enum SpecIssue {
    Warning(String),
    Hint(String),
}

impl Display for SpecIssue {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let str = match self {
            SpecIssue::Warning(msg) => format!("warning: {}", msg),
            SpecIssue::Hint(msg) => format!("hint: {}", msg),
        };
        write!(f, "{}", str)
    }
}

pub struct Spec<T> {
    pub spec: T,
    pub issues: Vec<SpecIssue>,
}

fn to_model_name(name: &str) -> String {
    name.to_camel_case()
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
    pub fn from_db(database: &Database) -> Result<Spec<SchemaSpec>> {
        // Query to get a list of all the tables in the database
        const QUERY: &str =
            "SELECT table_name FROM information_schema.tables WHERE table_schema = 'public'";

        let mut issues = Vec::new();
        let mut table_specs = Vec::new();

        for row in database
            .create_client()?
            .query(QUERY, &[])
            .map_err(|e| anyhow!(e))?
        {
            let name: String = row.get("table_name");
            let mut table = TableSpec::from_db(database, &name)?;
            issues.append(&mut table.issues);
            table_specs.push(table.spec);
        }

        Ok(Spec {
            spec: SchemaSpec { table_specs },
            issues,
        })
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
    fn from_db(database: &Database, table_name: &str) -> Result<Spec<TableSpec>> {
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
        let mut issues = Vec::new();

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

            let mut column =
                ColumnSpec::from_db(database, &ref_table_name, &ref_column_name, true, None)?;
            issues.append(&mut column.issues);

            if let Some(spec) = column.spec {
                foreign_constraints.insert(
                    column_name.clone(),
                    PhysicalColumnType::ColumnReference {
                        ref_table_name: ref_table_name.clone(),
                        ref_column_name: ref_column_name.clone(),
                        ref_pk_type: Box::new(spec.db_type),
                    },
                );
            }
        }

        let mut column_specs = Vec::new();
        for row in db_client.query(columns_query.as_str(), &[])? {
            let name: String = row.get("column_name");
            let mut column = ColumnSpec::from_db(
                database,
                table_name,
                &name,
                primary_keys.contains(&name),
                foreign_constraints.get(&name).cloned(),
            )?;
            issues.append(&mut column.issues);

            if let Some(spec) = column.spec {
                column_specs.push(spec);
            }
        }

        // not a robust check
        if table_name.ends_with('s') {
            issues.push(SpecIssue::Hint(format!(
                "model name `{}` should be changed to singular",
                to_model_name(table_name)
            )));
        }

        Ok(Spec {
            spec: TableSpec {
                name: table_name.to_string(),
                column_specs,
            },
            issues,
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
            table_annot,
            to_model_name(&self.name),
            column_stmts
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
    ) -> Result<Spec<Option<ColumnSpec>>> {
        // Find all sequences in the database that are used for SERIAL (autoincrement) columns
        // e.g. an autoincrement column `id` in the table `users` will create a sequence called
        // `users_id_seq`
        let serial_columns_query = "SELECT relname FROM pg_class WHERE relkind = 'S'";

        let mut db_client = database.create_client()?;
        let mut issues = Vec::new();

        let db_type = match explicit_type {
            Some(t) => {
                if let PhysicalColumnType::ColumnReference { ref_table_name, .. } = &t {
                    issues.push(SpecIssue::Hint(format!(
                        "consider adding a field to `{ref_table_name}` of type `[{model_name}]` to create a one-to-many relationship", 
                        model_name=to_model_name(table_name),
                        ref_table_name=ref_table_name
                    )));
                }

                Some(t)
            }
            None => {
                // Query to find the type of the column and the # of dimensions if the type is an array
                let db_type_query = format!(
                    "
                    SELECT format_type(atttypid, atttypmod), attndims
                    FROM pg_attribute
                    WHERE attrelid = '{}'::regclass AND attname = '{}'",
                    table_name, column_name
                );

                let rows = db_client.query(db_type_query.as_str(), &[])?;
                let row = rows.get(0).unwrap();

                let mut sql_type: String = row.get("format_type");
                let dims: i32 = row.get("attndims");

                // When querying array types, the number of dimensions is not correctly shown
                // e.g. a column declared as `INT[][][]` will be shown as `INT[]`
                // So we manually query how many dimensions the column has and append `[]` to
                // the type
                sql_type += &"[]".repeat(if dims == 0 { 0 } else { (dims - 1) as usize });
                match PhysicalColumnType::from_string(&sql_type) {
                    Ok(t) => Some(t),
                    Err(e) => {
                        issues.push(SpecIssue::Warning(format!(
                            "skipped column `{}.{}` ({})",
                            table_name,
                            column_name,
                            e.to_string()
                        )));
                        None
                    }
                }
            }
        };

        let serial_columns = db_client
            .query(serial_columns_query, &[])?
            .iter()
            .map(|row| -> String { row.get("relname") })
            .collect::<HashSet<_>>();

        Ok(Spec {
            spec: db_type.map(|db_type| ColumnSpec {
                name: column_name.to_string(),
                db_type,
                is_pk,
                is_autoincrement: serial_columns
                    .contains(&format!("{}_{}_seq", table_name, column_name)),
            }),
            issues,
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

        let (mut data_type, annots) = self.db_type.to_model();
        if let PhysicalColumnType::ColumnReference { .. } = self.db_type {
            data_type = to_model_name(&data_type);
        }

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
        } = self
            .db_type
            .to_sql(table_name, &self.name, self.is_autoincrement);
        let pk_str = if self.is_pk { " PRIMARY KEY" } else { "" };

        SQLStatement {
            statement: format!("\"{}\" {}{}", self.name, statement, pk_str),
            foreign_constraints,
        }
    }
}
