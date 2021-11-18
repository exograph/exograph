use std::collections::{HashMap, HashSet};
use std::fmt::{self, Display, Formatter};

use crate::sql::{column::PhysicalColumnType, database::Database};
use anyhow::{anyhow, Result};
use regex::Regex;

/// An SQL statement along with any foreign constraint statements that should follow after all the
/// statements have been executed.
pub struct SQLStatement {
    pub statement: String,
    pub foreign_constraints: Vec<String>,
}

impl Display for SQLStatement {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}\n{}",
            self.statement,
            self.foreign_constraints.join("\n")
        )
    }
}

/// An issue that a user may encounter when dealing with the database schema.
///
/// Used in `model import` command.
pub enum Issue {
    Warning(String),
    Hint(String),
}

impl Display for Issue {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let str = match self {
            Issue::Warning(msg) => format!("warning: {}", msg),
            Issue::Hint(msg) => format!("hint: {}", msg),
        };
        write!(f, "{}", str)
    }
}

/// Wraps a value with a list of issues.
pub struct WithIssues<T> {
    pub value: T,
    pub issues: Vec<Issue>,
}

/// Specification for the overall schema.
pub struct SchemaSpec {
    pub table_specs: Vec<TableSpec>,
}

impl SchemaSpec {
    /// Creates a new schema specification from an SQL database.
    pub fn from_db(database: &Database) -> Result<WithIssues<SchemaSpec>> {
        // Query to get a list of all the tables in the database
        const QUERY: &str =
            "SELECT table_name FROM information_schema.tables WHERE table_schema = 'public'";

        let mut issues = Vec::new();
        let mut table_specs = Vec::new();

        for row in database
            .get_client()?
            .query(QUERY, &[])
            .map_err(|e| anyhow!(e))?
        {
            let name: String = row.get("table_name");
            let mut table = TableSpec::from_db(database, &name)?;
            issues.append(&mut table.issues);
            table_specs.push(table.value);
        }

        Ok(WithIssues {
            value: SchemaSpec { table_specs },
            issues,
        })
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
pub struct TableSpec {
    pub name: String,
    pub column_specs: Vec<ColumnSpec>,
}

impl TableSpec {
    /// Creates a new table specification from an SQL table.
    pub fn from_db(database: &Database, table_name: &str) -> Result<WithIssues<TableSpec>> {
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

        let mut db_client = database.get_client()?;
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

            if let Some(spec) = column.value {
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

            if let Some(spec) = column.value {
                column_specs.push(spec);
            }
        }

        Ok(WithIssues {
            value: TableSpec {
                name: table_name.to_string(),
                column_specs,
            },
            issues,
        })
    }

    /// Converts the table specification to SQL statements.
    pub fn to_sql(&self) -> SQLStatement {
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
pub struct ColumnSpec {
    pub table_name: String,
    pub column_name: String,
    pub db_type: PhysicalColumnType,
    pub is_pk: bool,
    pub is_autoincrement: bool,
    pub is_nullable: bool,
}

impl ColumnSpec {
    /// Creates a new column specification from an SQL column.
    ///
    /// If the column references another table's column, the column's type can be specified with
    /// `explicit_type`.
    pub fn from_db(
        database: &Database,
        table_name: &str,
        column_name: &str,
        is_pk: bool,
        explicit_type: Option<PhysicalColumnType>,
    ) -> Result<WithIssues<Option<ColumnSpec>>> {
        // Find all sequences in the database that are used for SERIAL (autoincrement) columns
        // e.g. an autoincrement column `id` in the table `users` will create a sequence called
        // `users_id_seq`
        let serial_columns_query = "SELECT relname FROM pg_class WHERE relkind = 'S'";

        let mut db_client = database.get_client()?;
        let mut issues = Vec::new();

        let db_type = match explicit_type {
            Some(t) => Some(t),
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
                        issues.push(Issue::Warning(format!(
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

        let db_not_null_query = format!(
            "
            SELECT attnotnull
            FROM pg_attribute
            WHERE attrelid = '{}'::regclass AND attname = '{}'",
            table_name, column_name
        );

        let not_null: bool = db_client
            .query::<str>(db_not_null_query.as_str(), &[])?
            .get(0)
            .map(|row| row.get("attnotnull"))
            .unwrap();

        let serial_columns = db_client
            .query(serial_columns_query, &[])?
            .iter()
            .map(|row| -> String { row.get("relname") })
            .collect::<HashSet<_>>();

        Ok(WithIssues {
            value: db_type.map(|db_type| ColumnSpec {
                table_name: table_name.to_owned(),
                column_name: column_name.to_owned(),
                db_type,
                is_pk,
                is_autoincrement: serial_columns
                    .contains(&format!("{}_{}_seq", table_name, column_name)),
                is_nullable: !not_null,
            }),
            issues,
        })
    }

    /// Converts the column specification to SQL statements.
    pub fn to_sql(&self, table_name: &str) -> SQLStatement {
        let SQLStatement {
            statement,
            foreign_constraints,
        } = self
            .db_type
            .to_sql(table_name, &self.column_name, self.is_autoincrement);
        let pk_str = if self.is_pk { " PRIMARY KEY" } else { "" };
        let not_null_str = if !self.is_nullable && !self.is_pk {
            // primary keys are implied to be not null
            " NOT NULL"
        } else {
            ""
        };

        SQLStatement {
            statement: format!(
                "\"{}\" {}{}{}",
                self.column_name, statement, pk_str, not_null_str
            ),
            foreign_constraints,
        }
    }
}
