use std::collections::{HashMap, HashSet};
use std::fmt::{self, Display, Formatter};

use crate::sql::column::PhysicalColumnType;
use crate::{PhysicalColumn, PhysicalTable};
use anyhow::{anyhow, Result};
use deadpool_postgres::Client;
use regex::Regex;

/// An SQL statement along with any foreign constraint statements that should follow after all the
/// statements have been executed.
#[derive(Default)]
pub struct SQLStatement {
    pub statement: String,
    pub foreign_constraints_statements: Vec<String>,
}

/// An execution unit of SQL, representing an operation that can create or destroy resources.
#[derive(Debug)]
pub enum SQLOp<'a> {
    CreateTable {
        table: &'a PhysicalTable,
    },
    DeleteTable {
        table: &'a PhysicalTable,
    },

    CreateColumn {
        table: &'a PhysicalTable,
        column: &'a PhysicalColumn,
    },
    DeleteColumn {
        table: &'a PhysicalTable,
        column: &'a PhysicalColumn,
    },

    CreateExtension {
        extension: String,
    },
    RemoveExtension {
        extension: String,
    },
}

impl SQLOp<'_> {
    pub fn to_sql(&self) -> SQLStatement {
        match self {
            SQLOp::CreateTable { table } => table.to_sql(),
            SQLOp::DeleteTable { table } => SQLStatement {
                statement: format!("DROP TABLE \"{}\";", table.name),
                foreign_constraints_statements: vec![],
            },
            SQLOp::CreateColumn { table, column } => {
                let column = column.to_sql(&table.name);

                SQLStatement {
                    statement: format!("ALTER TABLE \"{}\" ADD {};", table.name, column.statement),
                    foreign_constraints_statements: column.foreign_constraints_statements,
                }
            }
            SQLOp::DeleteColumn { table, column } => SQLStatement {
                statement: format!(
                    "ALTER TABLE \"{}\" DROP COLUMN \"{}\";",
                    table.name, column.column_name
                ),
                ..Default::default()
            },
            SQLOp::CreateExtension { extension } => SQLStatement {
                statement: format!("CREATE EXTENSION \"{}\";", extension),
                ..Default::default()
            },
            SQLOp::RemoveExtension { extension } => SQLStatement {
                statement: format!("DROP EXTENSION \"{}\";", extension),
                ..Default::default()
            },
        }
    }
}

impl Display for SQLStatement {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}\n{}",
            self.statement,
            self.foreign_constraints_statements.join("\n")
        )
    }
}

/// An issue that a user may encounter when dealing with the database schema.
///
/// Used in `model import` command.
#[derive(Debug)]
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
#[derive(Debug)]
pub struct WithIssues<T> {
    pub value: T,
    pub issues: Vec<Issue>,
}

/// Specification for the overall schema.
pub struct SchemaSpec {
    pub table_specs: Vec<PhysicalTable>,
    pub required_extensions: HashSet<String>,
}

impl SchemaSpec {
    /// Creates a new schema specification from an SQL database.
    pub async fn from_db(client: &Client) -> Result<WithIssues<SchemaSpec>> {
        // Query to get a list of all the tables in the database
        const QUERY: &str =
            "SELECT table_name FROM information_schema.tables WHERE table_schema = 'public'";

        let mut issues = Vec::new();
        let mut table_specs = Vec::new();
        let mut required_extensions = HashSet::new();

        for row in client.query(QUERY, &[]).await.map_err(|e| anyhow!(e))? {
            let name: String = row.get("table_name");
            let mut table = PhysicalTable::from_db(client, &name).await?;
            issues.append(&mut table.issues);
            table_specs.push(table.value);
        }

        for table_spec in table_specs.iter() {
            required_extensions = required_extensions
                .union(&table_spec.get_required_extensions())
                .cloned()
                .collect();
        }

        Ok(WithIssues {
            value: SchemaSpec {
                table_specs,
                required_extensions,
            },
            issues,
        })
    }

    /// Merges the schema specification into a single SQL statement.
    pub fn to_sql_string(&self) -> String {
        let mut ops = Vec::new();

        self.required_extensions.iter().for_each(|ext| {
            ops.push(SQLOp::CreateExtension {
                extension: ext.to_owned(),
            });
        });

        self.table_specs.iter().for_each(|t| {
            ops.push(SQLOp::CreateTable { table: t });
        });

        let statements: Vec<String> = ops
            .into_iter()
            .map(|op| format!("{}", op.to_sql()))
            .collect();

        statements.join("\n")
    }
}

impl PhysicalTable {
    /// Creates a new table specification from an SQL table.
    pub async fn from_db(client: &Client, table_name: &str) -> Result<WithIssues<PhysicalTable>> {
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

        let mut issues = Vec::new();

        // Get all the constraints in the table
        let constraints = client
            .query(constraints_query.as_str(), &[])
            .await?
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
                PhysicalColumn::from_db(client, &ref_table_name, &ref_column_name, true, None)
                    .await?;
            issues.append(&mut column.issues);

            if let Some(spec) = column.value {
                foreign_constraints.insert(
                    column_name.clone(),
                    PhysicalColumnType::ColumnReference {
                        ref_table_name: ref_table_name.clone(),
                        ref_column_name: ref_column_name.clone(),
                        ref_pk_type: Box::new(spec.typ),
                    },
                );
            }
        }

        let mut columns = Vec::new();
        for row in client.query(columns_query.as_str(), &[]).await? {
            let name: String = row.get("column_name");
            let mut column = PhysicalColumn::from_db(
                client,
                table_name,
                &name,
                primary_keys.contains(&name),
                foreign_constraints.get(&name).cloned(),
            )
            .await?;
            issues.append(&mut column.issues);

            if let Some(spec) = column.value {
                columns.push(spec);
            }
        }

        Ok(WithIssues {
            value: PhysicalTable {
                name: table_name.to_string(),
                columns,
            },
            issues,
        })
    }

    /// Converts the table specification to SQL statements.
    pub fn to_sql(&self) -> SQLStatement {
        let mut foreign_constraints = Vec::new();
        let column_stmts: String = self
            .columns
            .iter()
            .map(|c| {
                let mut s = c.to_sql(&self.name);
                foreign_constraints.append(&mut s.foreign_constraints_statements);
                s.statement
            })
            .collect::<Vec<_>>()
            .join(",\n\t");

        let named_unique_constraints = self.columns.iter().fold(HashMap::new(), |mut map, c| {
            {
                for name in c.unique_constraints.iter() {
                    let entry: &mut Vec<String> = map.entry(name).or_insert_with(Vec::new);
                    (*entry).push(c.column_name.clone());
                }
            }
            map
        });

        for (unique_constraint_name, columns) in named_unique_constraints.iter() {
            let columns_part = columns
                .iter()
                .map(|c| format!("\"{}\"", c))
                .collect::<Vec<_>>()
                .join(", ");

            foreign_constraints.push(format!(
                "ALTER TABLE \"{}\" ADD CONSTRAINT \"{}\" UNIQUE ({});",
                self.name, unique_constraint_name, columns_part
            ));
        }

        SQLStatement {
            statement: format!("CREATE TABLE \"{}\" (\n\t{}\n);", self.name, column_stmts),
            foreign_constraints_statements: foreign_constraints,
        }
    }

    /// Get any extensions this table may depend on.
    pub fn get_required_extensions(&self) -> HashSet<String> {
        let mut required_extensions = HashSet::new();

        for col_spec in self.columns.iter() {
            if let PhysicalColumnType::Uuid = col_spec.typ {
                required_extensions.insert("pgcrypto".to_string());
            }
        }

        required_extensions
    }
}

impl PhysicalColumn {
    /// Creates a new column specification from an SQL column.
    ///
    /// If the column references another table's column, the column's type can be specified with
    /// `explicit_type`.
    pub async fn from_db(
        client: &Client,
        table_name: &str,
        column_name: &str,
        is_pk: bool,
        explicit_type: Option<PhysicalColumnType>,
    ) -> Result<WithIssues<Option<PhysicalColumn>>> {
        // Find all sequences in the database that are used for SERIAL (autoincrement) columns
        // e.g. an autoincrement column `id` in the table `users` will create a sequence called
        // `users_id_seq`
        let serial_columns_query = "SELECT relname FROM pg_class WHERE relkind = 'S'";

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

                let rows = client.query(db_type_query.as_str(), &[]).await?;
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
                            table_name, column_name, e
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

        let not_null: bool = client
            .query::<str>(db_not_null_query.as_str(), &[])
            .await?
            .get(0)
            .map(|row| row.get("attnotnull"))
            .unwrap();

        let serial_columns = client
            .query(serial_columns_query, &[])
            .await?
            .iter()
            .map(|row| -> String { row.get("relname") })
            .collect::<HashSet<_>>();

        let is_autoincrement =
            serial_columns.contains(&format!("{}_{}_seq", table_name, column_name));

        let default_value = if is_autoincrement {
            // if this column is autoincrement, then default value will be populated
            // with an invocation of nextval()
            //
            // clear it to normalize the column
            None
        } else {
            let db_type_query = format!(
                "
                SELECT pg_get_expr(pg_attrdef.adbin, pg_attrdef.adrelid)
                FROM pg_attrdef
                INNER JOIN pg_attribute
                ON pg_attrdef.adnum = pg_attribute.attnum
                AND pg_attribute.attrelid = '{}'::regclass
                AND pg_attribute.attname = '{}'",
                table_name, column_name
            );

            let rows = client.query(db_type_query.as_str(), &[]).await?;
            rows.get(0).map(|row| row.get("pg_get_expr"))
        };

        Ok(WithIssues {
            value: db_type.map(|typ| PhysicalColumn {
                table_name: table_name.to_owned(),
                column_name: column_name.to_owned(),
                typ,
                is_pk,
                is_autoincrement,
                is_nullable: !not_null,
                unique_constraints: vec![], // TODO: transfer unique constraints from db
                default_value,
            }),
            issues,
        })
    }

    /// Converts the column specification to SQL statements.
    pub fn to_sql(&self, table_name: &str) -> SQLStatement {
        let SQLStatement {
            statement,
            foreign_constraints_statements: foreign_constraints,
        } = self
            .typ
            .to_sql(table_name, &self.column_name, self.is_autoincrement);
        let pk_str = if self.is_pk { " PRIMARY KEY" } else { "" };
        let not_null_str = if !self.is_nullable && !self.is_pk {
            // primary keys are implied to be not null
            " NOT NULL"
        } else {
            ""
        };
        let default_value_part = if let Some(default_value) = self.default_value.as_ref() {
            format!(" DEFAULT {}", default_value)
        } else {
            "".to_string()
        };

        SQLStatement {
            statement: format!(
                "\"{}\" {}{}{}{}",
                self.column_name, statement, pk_str, not_null_str, default_value_part
            ),
            foreign_constraints_statements: foreign_constraints,
        }
    }
}
