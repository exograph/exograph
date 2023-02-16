use crate::database_error::DatabaseError;
use crate::{FloatBits, IntBits, PhysicalColumn, PhysicalColumnType};

use super::issue::{Issue, WithIssues};
use super::op::SchemaOp;
use super::statement::SchemaStatement;
use deadpool_postgres::Client;
use std::collections::HashSet;
use std::fmt::Write;

impl PhysicalColumn {
    pub fn diff<'a>(&'a self, new: &'a Self) -> Vec<SchemaOp<'a>> {
        let mut changes = vec![];
        let table_name_same = self.table_name == new.table_name;
        let column_name_same = self.column_name == new.column_name;
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
            changes.push(SchemaOp::DeleteColumn { column: self });
            changes.push(SchemaOp::CreateColumn { column: new });
        } else if !is_nullable_same {
            if new.is_nullable && !self.is_nullable {
                // drop NOT NULL constraint
                changes.push(SchemaOp::UnsetNotNull { column: self })
            } else {
                // add NOT NULL constraint
                changes.push(SchemaOp::SetNotNull { column: self })
            }
        } else if !default_value_same {
            match &new.default_value {
                Some(default_value) => {
                    changes.push(SchemaOp::SetColumnDefaultValue {
                        column: new,
                        default_value: default_value.clone(),
                    });
                }
                None => {
                    changes.push(SchemaOp::UnsetColumnDefaultValue { column: new });
                }
            }
        }

        changes
    }

    /// Creates a new column specification from an SQL column.
    ///
    /// If the column references another table's column, the column's type can be specified with
    /// `explicit_type`.
    pub(super) async fn from_db(
        client: &Client,
        table_name: &str,
        column_name: &str,
        is_pk: bool,
        explicit_type: Option<PhysicalColumnType>,
        unique_constraints: Vec<String>,
    ) -> Result<WithIssues<Option<PhysicalColumn>>, DatabaseError> {
        // Find all sequences in the database that are used for SERIAL (autoIncrement) columns
        // e.g. an autoIncrement column `id` in the table `users` will create a sequence called
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
                    WHERE attrelid = '{table_name}'::regclass AND attname = '{column_name}'"
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
                            "skipped column `{table_name}.{column_name}` ({e})"
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
            WHERE attrelid = '{table_name}'::regclass AND attname = '{column_name}'"
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

        let is_auto_increment = serial_columns.contains(&format!("{table_name}_{column_name}_seq"));

        let default_value = if is_auto_increment {
            // if this column is autoIncrement, then default value will be populated
            // with an invocation of nextval()
            //
            // clear it to normalize the column
            None
        } else {
            let db_query = format!(
                "
                SELECT column_default FROM information_schema.columns
                WHERE table_name='{table_name}' and column_name = '{column_name}'"
            );

            let rows = client.query(db_query.as_str(), &[]).await?;

            rows.get(0)
                .and_then(|row| row.try_get("column_default").ok())
        };

        Ok(WithIssues {
            value: db_type.map(|typ| PhysicalColumn {
                table_name: table_name.to_owned(),
                column_name: column_name.to_owned(),
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
    pub(super) fn to_sql(&self) -> SchemaStatement {
        let SchemaStatement {
            statement,
            post_statements,
            ..
        } = self
            .typ
            .to_sql(&self.table_name, &self.column_name, self.is_auto_increment);
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
                self.column_name, statement, pk_str, not_null_str, default_value_part
            ),
            pre_statements: vec![],
            post_statements,
        }
    }
}

impl PhysicalColumnType {
    pub(super) fn to_sql(
        &self,
        table_name: &str,
        column_name: &str,
        is_auto_increment: bool,
    ) -> SchemaStatement {
        match self {
            PhysicalColumnType::Int { bits } => SchemaStatement {
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

            PhysicalColumnType::Float { bits } => SchemaStatement {
                statement: match bits {
                    FloatBits::_24 => "REAL",
                    FloatBits::_53 => "DOUBLE PRECISION",
                }
                .to_owned(),
                pre_statements: vec![],
                post_statements: vec![],
            },

            PhysicalColumnType::Numeric { precision, scale } => SchemaStatement {
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

            PhysicalColumnType::String { length } => SchemaStatement {
                statement: if let Some(length) = length {
                    format!("VARCHAR({length})")
                } else {
                    "TEXT".to_owned()
                },
                pre_statements: vec![],
                post_statements: vec![],
            },

            PhysicalColumnType::Boolean => SchemaStatement {
                statement: "BOOLEAN".to_owned(),
                pre_statements: vec![],
                post_statements: vec![],
            },

            PhysicalColumnType::Timestamp {
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
                        PhysicalColumnType::Timestamp { .. } => "TIMESTAMP",
                        PhysicalColumnType::Time { .. } => "TIME",
                        _ => panic!(),
                    };

                    // e.g. "TIMESTAMP(3) WITH TIME ZONE"
                    format!("{typ}{precision_option} {timezone_option}")
                },
                pre_statements: vec![],
                post_statements: vec![],
            },

            PhysicalColumnType::Time { precision } => SchemaStatement {
                statement: if let Some(p) = precision {
                    format!("TIME({p})")
                } else {
                    "TIME".to_owned()
                },
                pre_statements: vec![],
                post_statements: vec![],
            },

            PhysicalColumnType::Date => SchemaStatement {
                statement: "DATE".to_owned(),
                pre_statements: vec![],
                post_statements: vec![],
            },

            PhysicalColumnType::Json => SchemaStatement {
                statement: "JSONB".to_owned(),
                pre_statements: vec![],
                post_statements: vec![],
            },

            PhysicalColumnType::Blob => SchemaStatement {
                statement: "BYTEA".to_owned(),
                pre_statements: vec![],
                post_statements: vec![],
            },

            PhysicalColumnType::Uuid => SchemaStatement {
                statement: "uuid".to_owned(),
                pre_statements: vec![],
                post_statements: vec![],
            },

            PhysicalColumnType::Array { typ } => {
                // 'unwrap' nested arrays all the way to the underlying primitive type

                let mut underlying_typ = typ;
                let mut dimensions = 1;

                while let PhysicalColumnType::Array { typ } = &**underlying_typ {
                    underlying_typ = typ;
                    dimensions += 1;
                }

                // build dimensions

                let mut dimensions_part = String::new();

                for _ in 0..dimensions {
                    write!(&mut dimensions_part, "[]").unwrap();
                }

                let mut sql_statement =
                    underlying_typ.to_sql(table_name, column_name, is_auto_increment);
                sql_statement.statement += &dimensions_part;
                sql_statement
            }

            PhysicalColumnType::ColumnReference {
                ref_table_name,
                ref_pk_type,
                ..
            } => {
                let mut sql_statement =
                    ref_pk_type.to_sql(table_name, column_name, is_auto_increment);
                let foreign_constraint = format!(
                    r#"ALTER TABLE "{table_name}" ADD CONSTRAINT "{table_name}_{column_name}_fk" FOREIGN KEY ("{column_name}") REFERENCES "{ref_table_name}";"#,
                );

                sql_statement.post_statements.push(foreign_constraint);
                sql_statement
            }
        }
    }
}
