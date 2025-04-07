// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::collections::HashMap;
use std::fmt::Write;

use crate::database_error::DatabaseError;
use crate::sql::connect::database_client::DatabaseClient;
use crate::{
    Database, FloatBits, IntBits, ManyToOne, PhysicalColumn, PhysicalColumnType, PhysicalTableName,
};

use super::issue::{Issue, WithIssues};
use super::op::SchemaOp;
use super::statement::SchemaStatement;
use super::table_spec::TableSpec;
use regex::Regex;

#[derive(Debug, Clone, PartialEq)]
pub struct ColumnSpec {
    pub name: String,
    pub typ: ColumnTypeSpec,
    pub is_pk: bool,
    pub is_auto_increment: bool,
    pub is_nullable: bool,
    pub unique_constraints: Vec<String>,
    pub default_value: Option<String>,
    // A name that can be used to group columns together (for example to generate a foreign key constraint name for composite primary keys)
    pub group_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ColumnReferenceSpec {
    pub foreign_table_name: PhysicalTableName,
    pub foreign_pk_column_name: String,
    pub foreign_pk_type: Box<ColumnTypeSpec>,
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
    Vector {
        size: usize,
    },
    Array {
        typ: Box<ColumnTypeSpec>,
    },
    ColumnReference(ColumnReferenceSpec),
    Float {
        bits: FloatBits,
    },
    Numeric {
        precision: Option<usize>,
        scale: Option<usize>,
    },
}

const COLUMNS_TYPE_QUERY: &str = "
  SELECT pg_class.relname as table_name, attname as column_name, format_type(atttypid, atttypmod), attndims, attnotnull FROM pg_attribute 
    LEFT JOIN pg_class ON pg_attribute.attrelid = pg_class.oid 
    LEFT JOIN pg_namespace ON pg_class.relnamespace = pg_namespace.oid
  WHERE attnum > 0 AND attisdropped = false AND pg_namespace.nspname = $1";

const COLUMNS_DEFAULT_QUERY: &str = r#"
    SELECT 
        table_name,
        column_name,
        column_default,
        pg_get_serial_sequence('"' || $1 || '"."' || table_name || '"', column_name) IS NOT NULL AS is_auto_increment
    FROM 
        information_schema.columns
    WHERE 
        table_schema = $1;
"#;

const TEXT_TYPE_CAST_PREFIX: &str = "'::text";
const CHARACTER_VARYING_TYPE_CAST_PREFIX: &str = "'::character varying";

impl ColumnSpec {
    /// Creates a new column specification from an SQL column.
    ///
    /// If the column references another table's column, the column's type can be specified with
    /// `explicit_type`.
    pub async fn from_live_db(
        table_name: &PhysicalTableName,
        column_name: &str,
        is_pk: bool,
        explicit_type: Option<ColumnTypeSpec>,
        unique_constraints: Vec<String>,
        group_name: Option<String>,
        column_attributes: &HashMap<PhysicalTableName, HashMap<String, ColumnAttribute>>,
    ) -> Result<WithIssues<Option<ColumnSpec>>, DatabaseError> {
        let table_attributes = column_attributes
            .get(table_name)
            .ok_or(DatabaseError::Generic(format!(
                "Table `{}` not found",
                table_name.fully_qualified_name()
            )))?;

        let ColumnAttribute {
            is_auto_increment,
            default_value,
            not_null,
            db_type,
        } = table_attributes
            .get(column_name)
            .ok_or(DatabaseError::Generic(format!(
                "Column `{}` not found in table `{}`",
                column_name,
                table_name.fully_qualified_name()
            )))?;

        let db_type = explicit_type.or(db_type.clone());

        Ok(WithIssues {
            value: db_type.map(|typ| ColumnSpec {
                name: column_name.to_owned(),
                typ,
                is_pk,
                is_auto_increment: *is_auto_increment,
                is_nullable: !not_null,
                unique_constraints,
                default_value: default_value.clone(),
                group_name,
            }),
            issues: vec![],
        })
    }

    /// Converts the column specification to SQL statements.
    pub(super) fn to_sql(&self, attach_pk_column_to_column_stmt: bool) -> SchemaStatement {
        let SchemaStatement {
            statement,
            post_statements,
            ..
        } = self.typ.to_sql(self.is_auto_increment);
        let pk_str = if self.is_pk && attach_pk_column_to_column_stmt {
            " PRIMARY KEY"
        } else {
            ""
        };
        let not_null_str = if !self.is_nullable && !self.is_pk {
            // primary keys are implied to be not null
            " NOT NULL"
        } else {
            ""
        };
        let default_value_part = if let Some(default_value) = self.default_value_sql() {
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

    pub fn default_value_sql(&self) -> Option<String> {
        self.default_value.as_ref().map(|default_value| {
            if let ColumnTypeSpec::String { max_length } = &self.typ {
                let cast = if let Some(max_length) = max_length {
                    format!("{CHARACTER_VARYING_TYPE_CAST_PREFIX}({max_length})")
                } else {
                    TEXT_TYPE_CAST_PREFIX.to_string()
                };
                format!(" '{default_value}{cast}")
            } else {
                format!(" {default_value}")
            }
        })
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
        let default_value_same = self.default_value == new.default_value;

        if !(table_name_same && column_name_same) {
            panic!("Diffing columns must have the same table name and column name");
        }

        // If the column type differs only in reference type, that is taken care by table-level migration
        if (!type_same && !self.differs_only_in_reference_column(new))
            || !is_pk_same
            || !is_auto_increment_same
        {
            changes.push(SchemaOp::DeleteColumn {
                table: self_table,
                column: self,
            });
            changes.push(SchemaOp::CreateColumn {
                table: new_table,
                column: new,
            })
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
            match new.default_value_sql() {
                Some(default_value) => {
                    changes.push(SchemaOp::SetColumnDefaultValue {
                        table: new_table,
                        column: new,
                        default_value,
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
                Some(ManyToOne { column_pairs, .. }) => {
                    let foreign_pk_column = column_pairs
                        .iter()
                        .find(|cp| cp.self_column_id == column_id)
                        .unwrap()
                        .foreign_column_id
                        .get_column(database);
                    let foreign_table = database.get_table(foreign_pk_column.table_id);

                    ColumnTypeSpec::ColumnReference(ColumnReferenceSpec {
                        foreign_table_name: foreign_table.name.clone(),
                        foreign_pk_column_name: foreign_pk_column.name.clone(),
                        foreign_pk_type: Box::new(ColumnTypeSpec::from_physical(
                            foreign_pk_column.typ.clone(),
                        )),
                    })
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
            group_name: column.group_name,
        }
    }

    fn differs_only_in_reference_column(&self, new: &Self) -> bool {
        match (&self.typ, &new.typ) {
            (ColumnTypeSpec::ColumnReference { .. }, ColumnTypeSpec::ColumnReference { .. }) => {
                (self.typ != new.typ) && {
                    Self {
                        typ: ColumnTypeSpec::Int { bits: IntBits::_16 },
                        group_name: None,
                        ..self.clone()
                    } == Self {
                        typ: ColumnTypeSpec::Int { bits: IntBits::_16 },
                        group_name: None,
                        ..new.clone()
                    }
                }
            }
            _ => false,
        }
    }

    // The default value that should be used in the model.
    // For text, the default value from the database is a string with a type cast to text (such as `'foo'::text`),
    // so we need to remove the type cast.
    pub fn model_default_value(&self) -> Option<String> {
        self.default_value.as_ref().map(|default_value| {
            if matches!(self.typ, ColumnTypeSpec::String { .. }) {
                format!("\"{default_value}\"")
            } else if default_value == "gen_random_uuid()" {
                "generate_uuid()".to_string()
            } else if default_value == "CURRENT_TIMESTAMP" || default_value == "CURRENT_DATE" {
                "now()".to_string()
            } else {
                default_value.clone()
            }
        })
    }

    /// Compute column attributes from all tables in the given schema
    pub async fn query_column_attributes(
        client: &DatabaseClient,
        schema_name: &str,
        issues: &mut Vec<Issue>,
    ) -> Result<HashMap<PhysicalTableName, HashMap<String, ColumnAttribute>>, DatabaseError> {
        let mut map = HashMap::new();

        for row in client.query(COLUMNS_DEFAULT_QUERY, &[&schema_name]).await? {
            let table_name: String = row.get("table_name");
            let column_name: String = row.get("column_name");
            let is_auto_increment = row.get("is_auto_increment");

            let table_name = PhysicalTableName::new_with_schema_name(table_name, schema_name);

            // If this column is autoIncrement, then default value will be populated
            // with an invocation of nextval(). Thus, we need to clear it to normalize the column
            let default_value = if is_auto_increment {
                None
            } else {
                let default_value: Option<String> = row.get("column_default");
                default_value.map(|mut default_value| {
                    // The default value from the database is a string with a type cast to text.

                    if default_value.ends_with(TEXT_TYPE_CAST_PREFIX) {
                        default_value = default_value
                            [1..default_value.len() - TEXT_TYPE_CAST_PREFIX.len()]
                            .to_string();
                    } else if default_value.ends_with(CHARACTER_VARYING_TYPE_CAST_PREFIX) {
                        default_value = default_value
                            [1..default_value.len() - CHARACTER_VARYING_TYPE_CAST_PREFIX.len()]
                            .to_string();
                    }

                    default_value
                })
            };

            let table_attributes = map.entry(table_name).or_insert_with(HashMap::new);

            table_attributes.insert(
                column_name,
                ColumnAttribute {
                    is_auto_increment,
                    default_value,
                    db_type: None,
                    not_null: true,
                },
            );
        }

        for row in client.query(COLUMNS_TYPE_QUERY, &[&schema_name]).await? {
            let table_name: String = row.get("table_name");
            let column_name: String = row.get("column_name");
            let not_null: bool = row.get("attnotnull");

            let table_name = PhysicalTableName::new_with_schema_name(table_name, schema_name);

            let db_type = {
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
            };

            let table_attributes = map.entry(table_name).or_insert_with(HashMap::new);

            table_attributes.entry(column_name).and_modify(|info| {
                info.db_type = db_type;
                info.not_null = not_null;
            });
        }

        Ok(map)
    }
}

#[derive(Debug)]
pub struct ColumnAttribute {
    pub is_auto_increment: bool,
    pub default_value: Option<String>,
    pub db_type: Option<ColumnTypeSpec>,
    pub not_null: bool,
}

impl ColumnTypeSpec {
    /// Create a new physical column type given the SQL type string. This is used to reverse-engineer
    /// a database schema to a Exograph model.
    pub fn from_string(s: &str) -> Result<ColumnTypeSpec, DatabaseError> {
        let s = s.to_uppercase();

        let vector_re = Regex::new(r"VECTOR\((\d+)\)").unwrap();
        if let Some(captures) = vector_re.captures(&s) {
            let size = captures.get(1).unwrap().as_str().parse().unwrap();
            return Ok(ColumnTypeSpec::Vector { size });
        }

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
                "JSON" => ColumnTypeSpec::Json,
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
                        let captures = regex.captures(s);

                        let (precision, scale) = match captures {
                            Some(captures) => {
                                let precision = captures
                                    .name("precision")
                                    .and_then(|s| s.as_str().parse().ok());
                                let scale =
                                    captures.name("scale").and_then(|s| s.as_str().parse().ok());

                                (precision, scale)
                            }
                            None => (None, None),
                        };

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
            ColumnTypeSpec::Vector { size } => PhysicalColumnType::Vector { size: *size },
            ColumnTypeSpec::Array { typ } => PhysicalColumnType::Array {
                typ: Box::new(typ.to_database_type()),
            },
            ColumnTypeSpec::ColumnReference(ColumnReferenceSpec {
                foreign_pk_type, ..
            }) => foreign_pk_type.to_database_type(),
            ColumnTypeSpec::Float { bits } => PhysicalColumnType::Float { bits: *bits },
            ColumnTypeSpec::Numeric { precision, scale } => PhysicalColumnType::Numeric {
                precision: *precision,
                scale: *scale,
            },
        }
    }

    pub(super) fn to_sql(&self, is_auto_increment: bool) -> SchemaStatement {
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

            Self::Vector { size, .. } => SchemaStatement {
                statement: format!("Vector({size})"),
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

                let mut sql_statement = underlying_typ.to_sql(is_auto_increment);
                sql_statement.statement += &dimensions_part;
                sql_statement
            }

            Self::ColumnReference(ColumnReferenceSpec {
                foreign_pk_type, ..
            }) => foreign_pk_type.to_sql(is_auto_increment),
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
            PhysicalColumnType::Vector { size } => ColumnTypeSpec::Vector { size },
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
