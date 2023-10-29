// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::fmt::Write;

use crate::database_error::DatabaseError;
use crate::{
    Database, FloatBits, IntBits, ManyToOne, PhysicalColumn, PhysicalColumnType, PhysicalTableName,
};

use super::issue::{Issue, WithIssues};
use super::op::SchemaOp;
use super::statement::SchemaStatement;
use super::table_spec::TableSpec;
use deadpool_postgres::Client;
use regex::Regex;
use std::collections::HashSet;

#[derive(Debug, Clone)]
pub struct ColumnSpec {
    pub name: String,
    pub typ: ColumnTypeSpec,
    pub is_pk: bool,
    pub is_auto_increment: bool,
    pub is_nullable: bool,
    pub unique_constraints: Vec<String>,
    pub default_value: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ColumnTypeSpec {
    Int {
        bits: IntBits,
    },
    String {
        max_length: Option<usize>,
    },
    Boolean,
    Timestamp {
        timezone: bool,
        precision: Option<usize>,
    },
    Date,
    Time {
        precision: Option<usize>,
    },
    Json,
    Blob,
    Uuid,
    Array {
        typ: Box<ColumnTypeSpec>,
    },
    ColumnReference {
        foreign_table_name: PhysicalTableName,
        foreign_pk_column_name: String,
        foreign_pk_type: Box<ColumnTypeSpec>,
    },
    Float {
        bits: FloatBits,
    },
    Numeric {
        precision: Option<usize>,
        scale: Option<usize>,
    },
}

impl ColumnSpec {
    /// Creates a new column specification from an SQL column.
    ///
    /// If the column references another table's column, the column's type can be specified with
    /// `explicit_type`.
    pub async fn from_live_db(
        client: &Client,
        table_name: &PhysicalTableName,
        column_name: &str,
        is_pk: bool,
        explicit_type: Option<ColumnTypeSpec>,
        unique_constraints: Vec<String>,
    ) -> Result<WithIssues<Option<ColumnSpec>>, DatabaseError> {
        let mut issues = Vec::new();

        let db_type = match explicit_type {
            Some(t) => Some(t),
            None => {
                // Query to find the type of the column and the # of dimensions if the type is an array
                let db_type_query = format!(
                    "
                    SELECT format_type(atttypid, atttypmod), attndims
                    FROM pg_attribute
                    WHERE attrelid = '{}'::regclass AND attname = '{column_name}'",
                    table_name.fully_qualified_name()
                );

                let rows = client.query(db_type_query.as_str(), &[]).await?;
                let row = rows.get(0).unwrap();

                let mut sql_type: String = row.get("format_type");
                let dims = {
                    // depending on the version of postgres, the type of `attndims` is either `i16`
                    // or `i32` (postgres type is `int2`` or `int4``), so try both
                    let dims: Result<i32, _> = row.try_get("attndims");

                    match dims {
                        Ok(dims) => dims,
                        Err(_) => {
                            let dims: i16 = row.get("attndims");
                            dims as i32
                        }
                    }
                };

                // When querying array types, the number of dimensions is not correctly shown
                // e.g. a column declared as `INT[][][]` will be shown as `INT[]`
                // So we manually query how many dimensions the column has and append `[]` to
                // the type
                sql_type += &"[]".repeat(if dims == 0 { 0 } else { (dims - 1) as usize });
                match ColumnTypeSpec::from_string(&sql_type) {
                    Ok(t) => Some(t),
                    Err(e) => {
                        issues.push(Issue::Warning(format!(
                            "skipped column `{}.{column_name}` ({e})",
                            table_name.fully_qualified_name()
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
            WHERE attrelid = '{}'::regclass AND attname = '{column_name}'",
            table_name.fully_qualified_name()
        );

        let not_null: bool = client
            .query::<str>(db_not_null_query.as_str(), &[])
            .await?
            .get(0)
            .map(|row| row.get("attnotnull"))
            .unwrap();

        // Find all sequences in the database that are used for SERIAL (autoIncrement) columns
        // e.g. an autoIncrement column `id` in the table `users` will create a sequence called
        // `users_id_seq`
        let serial_columns_query =
            format!(
                "SELECT relname FROM pg_class WHERE relkind = 'S' and relnamespace = '{}'::regnamespace", 
                table_name.schema.clone().unwrap_or("public".to_string())
            );

        let serial_columns = client
            .query(&serial_columns_query, &[])
            .await?
            .iter()
            .map(|row| -> String { row.get("relname") })
            .collect::<HashSet<_>>();

        // Note that the autogenerated sequence name doesn't have the schema name in it (however,
        // the `serial_columns_query` takes care of selecting from the correct schema)
        let is_auto_increment =
            serial_columns.contains(&format!("{}_{column_name}_seq", table_name.name));

        let default_value = if is_auto_increment {
            // if this column is autoIncrement, then default value will be populated
            // with an invocation of nextval()
            //
            // clear it to normalize the column
            None
        } else {
            let table_predicate = match table_name.schema {
                Some(ref schema) => format!(
                    "table_schema = '{}' AND table_name = '{}'",
                    schema, table_name.name
                ),
                None => format!("table_name = '{}'", table_name.name),
            };

            let db_query = format!(
                "
                SELECT column_default FROM information_schema.columns
                WHERE {table_predicate} and column_name = '{column_name}'"
            );

            let rows = client.query(db_query.as_str(), &[]).await?;

            rows.get(0)
                .and_then(|row| row.try_get("column_default").ok())
        };

        Ok(WithIssues {
            value: db_type.map(|typ| ColumnSpec {
                name: column_name.to_owned(),
                typ,
                is_pk,
                is_auto_increment,
                is_nullable: !not_null,
                unique_constraints,
                default_value,
            }),
            issues,
        })
    }

    /// Converts the column specification to SQL statements.
    pub(super) fn to_sql(&self, table_spec: &TableSpec) -> SchemaStatement {
        let SchemaStatement {
            statement,
            post_statements,
            ..
        } = self
            .typ
            .to_sql(table_spec, &self.name, self.is_auto_increment);
        let pk_str = if self.is_pk { " PRIMARY KEY" } else { "" };
        let not_null_str = if !self.is_nullable && !self.is_pk {
            // primary keys are implied to be not null
            " NOT NULL"
        } else {
            ""
        };
        let default_value_part = if let Some(default_value) = self.default_value.as_ref() {
            format!(" DEFAULT {default_value}")
        } else {
            "".to_string()
        };

        SchemaStatement {
            statement: format!(
                "\"{}\" {}{}{}{}",
                self.name, statement, pk_str, not_null_str, default_value_part
            ),
            pre_statements: vec![],
            post_statements,
        }
    }

    pub fn diff<'a>(
        &'a self,
        new: &'a Self,
        self_table: &'a TableSpec,
        new_table: &'a TableSpec,
    ) -> Vec<SchemaOp<'a>> {
        let mut changes = vec![];
        let table_name_same = self_table.sql_name() == new_table.sql_name();
        let column_name_same = self.name == new.name;
        let type_same = self.typ == new.typ;
        let is_pk_same = self.is_pk == new.is_pk;
        let is_auto_increment_same = self.is_auto_increment == new.is_auto_increment;
        let is_nullable_same = self.is_nullable == new.is_nullable;
        let _unique_constraints_same = self.unique_constraints == new.unique_constraints;
        let default_value_same = self.default_value == new.default_value;

        if !(table_name_same && column_name_same) {
            panic!("Diffing columns must have the same table name and column name");
        }

        if !(type_same || is_pk_same || is_auto_increment_same) {
            changes.push(SchemaOp::DeleteColumn {
                table: self_table,
                column: self,
            });
            changes.push(SchemaOp::CreateColumn {
                table: new_table,
                column: new,
            });
        } else if !is_nullable_same {
            if new.is_nullable && !self.is_nullable {
                // drop NOT NULL constraint
                changes.push(SchemaOp::UnsetNotNull {
                    table: self_table,
                    column: self,
                })
            } else {
                // add NOT NULL constraint
                changes.push(SchemaOp::SetNotNull {
                    table: self_table,
                    column: self,
                })
            }
        } else if !default_value_same {
            match &new.default_value {
                Some(default_value) => {
                    changes.push(SchemaOp::SetColumnDefaultValue {
                        table: new_table,
                        column: new,
                        default_value: default_value.clone(),
                    });
                }
                None => {
                    changes.push(SchemaOp::UnsetColumnDefaultValue {
                        table: new_table,
                        column: new,
                    });
                }
            }
        }

        changes
    }

    pub(crate) fn from_physical(column: PhysicalColumn, database: &Database) -> ColumnSpec {
        let typ = {
            let column_id = database
                .get_column_id(column.table_id, &column.name)
                .unwrap();
            let relation = column_id
                .get_mto_relation(database)
                .map(|relation_id| relation_id.deref(database));

            match relation {
                Some(ManyToOne {
                    foreign_pk_column_id,
                    ..
                }) => {
                    let foreign_pk_column = foreign_pk_column_id.get_column(database);
                    let foreign_table = database.get_table(foreign_pk_column.table_id);

                    ColumnTypeSpec::ColumnReference {
                        foreign_table_name: foreign_table.name.clone(),
                        foreign_pk_column_name: foreign_pk_column.name.clone(),
                        foreign_pk_type: Box::new(ColumnTypeSpec::from_physical(
                            foreign_pk_column.typ.clone(),
                        )),
                    }
                }
                None => ColumnTypeSpec::from_physical(column.typ),
            }
        };

        ColumnSpec {
            name: column.name,
            typ,
            is_pk: column.is_pk,
            is_auto_increment: column.is_auto_increment,
            is_nullable: column.is_nullable,
            unique_constraints: column.unique_constraints,
            default_value: column.default_value,
        }
    }
}

impl ColumnTypeSpec {
    /// Create a new physical column type given the SQL type string. This is used to reverse-engineer
    /// a database schema to a Exograph model.
    pub fn from_string(s: &str) -> Result<ColumnTypeSpec, DatabaseError> {
        let s = s.to_uppercase();

        match s.find('[') {
            // If the type contains `[`, then it's an array type
            Some(idx) => {
                let db_type = &s[..idx]; // The underlying data type (e.g. `INT` in `INT[][]`)
                let mut dims = &s[idx..]; // The array brackets (e.g. `[][]` in `INT[][]`)

                // Count how many `[]` exist in `dims` (how many dimensions does this array have)
                let mut count = 0;
                loop {
                    if !dims.is_empty() {
                        if dims.len() >= 2 && &dims[0..2] == "[]" {
                            dims = &dims[2..];
                            count += 1;
                        } else {
                            return Err(DatabaseError::Validation(format!("unknown type {s}")));
                        }
                    } else {
                        break;
                    }
                }

                // Wrap the underlying type with `ColumnTypeSpec::Array`
                let mut array_type = ColumnTypeSpec::Array {
                    typ: Box::new(ColumnTypeSpec::from_string(db_type)?),
                };
                for _ in 0..count - 1 {
                    array_type = ColumnTypeSpec::Array {
                        typ: Box::new(array_type),
                    };
                }
                Ok(array_type)
            }

            None => Ok(match s.as_str() {
                // TODO: not really correct...
                "SMALLSERIAL" => ColumnTypeSpec::Int { bits: IntBits::_16 },
                "SMALLINT" => ColumnTypeSpec::Int { bits: IntBits::_16 },
                "INT" => ColumnTypeSpec::Int { bits: IntBits::_32 },
                "INTEGER" => ColumnTypeSpec::Int { bits: IntBits::_32 },
                "SERIAL" => ColumnTypeSpec::Int { bits: IntBits::_32 },
                "BIGINT" => ColumnTypeSpec::Int { bits: IntBits::_64 },
                "BIGSERIAL" => ColumnTypeSpec::Int { bits: IntBits::_64 },

                "REAL" => ColumnTypeSpec::Float {
                    bits: FloatBits::_24,
                },
                "DOUBLE PRECISION" => ColumnTypeSpec::Float {
                    bits: FloatBits::_53,
                },

                "UUID" => ColumnTypeSpec::Uuid,
                "TEXT" => ColumnTypeSpec::String { max_length: None },
                "BOOLEAN" => ColumnTypeSpec::Boolean,
                "JSONB" => ColumnTypeSpec::Json,
                "BYTEA" => ColumnTypeSpec::Blob,
                s => {
                    // parse types with arguments
                    // TODO: more robust parsing

                    let get_num = |s: &str| {
                        s.chars()
                            .filter(|c| c.is_numeric())
                            .collect::<String>()
                            .parse::<usize>()
                            .ok()
                    };

                    if s.starts_with("CHARACTER VARYING")
                        || s.starts_with("VARCHAR")
                        || s.starts_with("CHAR")
                    {
                        ColumnTypeSpec::String {
                            max_length: get_num(s),
                        }
                    } else if s.starts_with("TIMESTAMP") {
                        ColumnTypeSpec::Timestamp {
                            precision: get_num(s),
                            timezone: s.contains("WITH TIME ZONE"),
                        }
                    } else if s.starts_with("TIME") {
                        ColumnTypeSpec::Time {
                            precision: get_num(s),
                        }
                    } else if s.starts_with("DATE") {
                        ColumnTypeSpec::Date
                    } else if s.starts_with("NUMERIC") {
                        let regex =
                            Regex::new("NUMERIC\\((?P<precision>\\d+),?(?P<scale>\\d+)?\\)")
                                .map_err(|_| {
                                    DatabaseError::Validation("Invalid numeric column spec".into())
                                })?;
                        let captures = regex.captures(s).unwrap();

                        let precision = captures
                            .name("precision")
                            .and_then(|s| s.as_str().parse().ok());
                        let scale = captures.name("scale").and_then(|s| s.as_str().parse().ok());

                        ColumnTypeSpec::Numeric { precision, scale }
                    } else {
                        return Err(DatabaseError::Validation(format!("unknown type {s}")));
                    }
                }
            }),
        }
    }

    pub fn to_database_type(&self) -> PhysicalColumnType {
        match self {
            ColumnTypeSpec::Int { bits } => PhysicalColumnType::Int { bits: *bits },
            ColumnTypeSpec::String { max_length } => PhysicalColumnType::String {
                max_length: *max_length,
            },
            ColumnTypeSpec::Boolean => PhysicalColumnType::Boolean,
            ColumnTypeSpec::Timestamp {
                timezone,
                precision,
            } => PhysicalColumnType::Timestamp {
                timezone: *timezone,
                precision: *precision,
            },
            ColumnTypeSpec::Date => PhysicalColumnType::Date,
            ColumnTypeSpec::Time { precision } => PhysicalColumnType::Time {
                precision: *precision,
            },
            ColumnTypeSpec::Json => PhysicalColumnType::Json,
            ColumnTypeSpec::Blob => PhysicalColumnType::Blob,
            ColumnTypeSpec::Uuid => PhysicalColumnType::Uuid,
            ColumnTypeSpec::Array { typ } => PhysicalColumnType::Array {
                typ: Box::new(typ.to_database_type()),
            },
            ColumnTypeSpec::ColumnReference {
                foreign_pk_type, ..
            } => foreign_pk_type.to_database_type(),
            ColumnTypeSpec::Float { bits } => PhysicalColumnType::Float { bits: *bits },
            ColumnTypeSpec::Numeric { precision, scale } => PhysicalColumnType::Numeric {
                precision: *precision,
                scale: *scale,
            },
        }
    }

    pub fn to_model(&self) -> (String, String) {
        match self {
            ColumnTypeSpec::Int { bits } => (
                "Int".to_string(),
                match bits {
                    IntBits::_16 => " @bits16",
                    IntBits::_32 => "",
                    IntBits::_64 => " @bits64",
                }
                .to_string(),
            ),

            ColumnTypeSpec::Float { bits } => (
                "Float".to_string(),
                match bits {
                    FloatBits::_24 => " @singlePrecision",
                    FloatBits::_53 => " @doublePrecision",
                }
                .to_owned(),
            ),

            ColumnTypeSpec::Numeric { precision, scale } => ("Numeric".to_string(), {
                let precision_part = precision
                    .map(|p| format!("@precision({p})"))
                    .unwrap_or_default();

                let scale_part = scale.map(|s| format!("@scale({s})")).unwrap_or_default();

                format!(" {precision_part} {scale_part}")
            }),

            ColumnTypeSpec::String { max_length } => (
                "String".to_string(),
                match max_length {
                    Some(max_length) => format!(" @maxLength({max_length})"),
                    None => "".to_string(),
                },
            ),

            ColumnTypeSpec::Boolean => ("Boolean".to_string(), "".to_string()),

            ColumnTypeSpec::Timestamp {
                timezone,
                precision,
            } => (
                if *timezone {
                    "Instant"
                } else {
                    "LocalDateTime"
                }
                .to_string(),
                match precision {
                    Some(precision) => format!(" @precision({precision})"),
                    None => "".to_string(),
                },
            ),

            ColumnTypeSpec::Time { precision } => (
                "LocalTime".to_string(),
                match precision {
                    Some(precision) => format!(" @precision({precision})"),
                    None => "".to_string(),
                },
            ),

            ColumnTypeSpec::Date => ("LocalDate".to_string(), "".to_string()),

            ColumnTypeSpec::Json => ("Json".to_string(), "".to_string()),
            ColumnTypeSpec::Blob => ("Blob".to_string(), "".to_string()),
            ColumnTypeSpec::Uuid => ("Uuid".to_string(), "".to_string()),

            ColumnTypeSpec::Array { typ } => {
                let (data_type, annotations) = typ.to_model();
                (format!("[{data_type}]"), annotations)
            }

            ColumnTypeSpec::ColumnReference {
                foreign_table_name, ..
            } => (foreign_table_name.name.clone(), "".to_string()),
        }
    }

    pub(super) fn to_sql(
        &self,
        table_spec: &TableSpec,
        column_name: &str,
        is_auto_increment: bool,
    ) -> SchemaStatement {
        match self {
            Self::Int { bits } => SchemaStatement {
                statement: {
                    if is_auto_increment {
                        match bits {
                            IntBits::_16 => "SMALLSERIAL",
                            IntBits::_32 => "SERIAL",
                            IntBits::_64 => "BIGSERIAL",
                        }
                    } else {
                        match bits {
                            IntBits::_16 => "SMALLINT",
                            IntBits::_32 => "INT",
                            IntBits::_64 => "BIGINT",
                        }
                    }
                }
                .to_owned(),
                pre_statements: vec![],
                post_statements: vec![],
            },

            Self::Float { bits } => SchemaStatement {
                statement: match bits {
                    FloatBits::_24 => "REAL",
                    FloatBits::_53 => "DOUBLE PRECISION",
                }
                .to_owned(),
                pre_statements: vec![],
                post_statements: vec![],
            },

            Self::Numeric { precision, scale } => SchemaStatement {
                statement: {
                    if let Some(p) = precision {
                        if let Some(s) = scale {
                            format!("NUMERIC({p}, {s})")
                        } else {
                            format!("NUMERIC({p})")
                        }
                    } else {
                        assert!(scale.is_none()); // can't have a scale and no precision
                        "NUMERIC".to_owned()
                    }
                },
                pre_statements: vec![],
                post_statements: vec![],
            },

            Self::String { max_length } => SchemaStatement {
                statement: if let Some(max_length) = max_length {
                    format!("VARCHAR({max_length})")
                } else {
                    "TEXT".to_owned()
                },
                pre_statements: vec![],
                post_statements: vec![],
            },

            Self::Boolean => SchemaStatement {
                statement: "BOOLEAN".to_owned(),
                pre_statements: vec![],
                post_statements: vec![],
            },

            Self::Timestamp {
                timezone,
                precision,
            } => SchemaStatement {
                statement: {
                    let timezone_option = if *timezone {
                        "WITH TIME ZONE"
                    } else {
                        "WITHOUT TIME ZONE"
                    };
                    let precision_option = if let Some(p) = precision {
                        format!("({p})")
                    } else {
                        String::default()
                    };

                    let typ = match self {
                        Self::Timestamp { .. } => "TIMESTAMP",
                        Self::Time { .. } => "TIME",
                        _ => panic!(),
                    };

                    // e.g. "TIMESTAMP(3) WITH TIME ZONE"
                    format!("{typ}{precision_option} {timezone_option}")
                },
                pre_statements: vec![],
                post_statements: vec![],
            },

            Self::Time { precision } => SchemaStatement {
                statement: if let Some(p) = precision {
                    format!("TIME({p})")
                } else {
                    "TIME".to_owned()
                },
                pre_statements: vec![],
                post_statements: vec![],
            },

            Self::Date => SchemaStatement {
                statement: "DATE".to_owned(),
                pre_statements: vec![],
                post_statements: vec![],
            },

            Self::Json => SchemaStatement {
                statement: "JSONB".to_owned(),
                pre_statements: vec![],
                post_statements: vec![],
            },

            Self::Blob => SchemaStatement {
                statement: "BYTEA".to_owned(),
                pre_statements: vec![],
                post_statements: vec![],
            },

            Self::Uuid => SchemaStatement {
                statement: "uuid".to_owned(),
                pre_statements: vec![],
                post_statements: vec![],
            },

            Self::Array { typ } => {
                // 'unwrap' nested arrays all the way to the underlying primitive type

                let mut underlying_typ = typ;
                let mut dimensions = 1;

                while let Self::Array { typ } = &**underlying_typ {
                    underlying_typ = typ;
                    dimensions += 1;
                }

                // build dimensions

                let mut dimensions_part = String::new();

                for _ in 0..dimensions {
                    write!(&mut dimensions_part, "[]").unwrap();
                }

                let mut sql_statement =
                    underlying_typ.to_sql(table_spec, column_name, is_auto_increment);
                sql_statement.statement += &dimensions_part;
                sql_statement
            }

            Self::ColumnReference {
                foreign_table_name,
                foreign_pk_type,
                ..
            } => {
                let mut sql_statement =
                    foreign_pk_type.to_sql(table_spec, column_name, is_auto_increment);

                let foreign_table_str = match &foreign_table_name.schema {
                    Some(schema_name) => {
                        format!("\"{}\".\"{}\"", schema_name, foreign_table_name.name)
                    }
                    None => format!("\"{}\"", foreign_table_name.name),
                };

                let constraint_name = format!(
                    "{}_{}_fk",
                    table_spec.name.fully_qualified_name_with_sep("_"),
                    column_name
                );

                let foreign_constraint = format!(
                    r#"ALTER TABLE {} ADD CONSTRAINT "{constraint_name}" FOREIGN KEY ("{column_name}") REFERENCES {foreign_table_str};"#,
                    table_spec.sql_name()
                );

                sql_statement.post_statements.push(foreign_constraint);
                sql_statement
            }
        }
    }

    pub fn from_physical(typ: PhysicalColumnType) -> ColumnTypeSpec {
        match typ {
            PhysicalColumnType::Int { bits } => ColumnTypeSpec::Int { bits },
            PhysicalColumnType::String { max_length } => ColumnTypeSpec::String { max_length },
            PhysicalColumnType::Boolean => ColumnTypeSpec::Boolean,
            PhysicalColumnType::Timestamp {
                timezone,
                precision,
            } => ColumnTypeSpec::Timestamp {
                timezone,
                precision,
            },
            PhysicalColumnType::Date => ColumnTypeSpec::Date,
            PhysicalColumnType::Time { precision } => ColumnTypeSpec::Time { precision },
            PhysicalColumnType::Json => ColumnTypeSpec::Json,
            PhysicalColumnType::Blob => ColumnTypeSpec::Blob,
            PhysicalColumnType::Uuid => ColumnTypeSpec::Uuid,
            PhysicalColumnType::Array { typ } => ColumnTypeSpec::Array {
                typ: Box::new(ColumnTypeSpec::from_physical(*typ)),
            },
            PhysicalColumnType::Float { bits } => ColumnTypeSpec::Float { bits },
            PhysicalColumnType::Numeric { precision, scale } => {
                ColumnTypeSpec::Numeric { precision, scale }
            }
        }
    }
}
