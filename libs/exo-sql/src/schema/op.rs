// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::collections::HashSet;

use crate::schema::constraint::sorted_comma_list;

use super::{column_spec::ColumnSpec, statement::SchemaStatement, table_spec::TableSpec};

/// An execution unit of SQL, representing an operation that can create or destroy resources.
#[derive(Debug)]
pub enum SchemaOp<'a> {
    CreateTable {
        table: &'a TableSpec,
    },
    DeleteTable {
        table: &'a TableSpec,
    },

    CreateColumn {
        table: &'a TableSpec,
        column: &'a ColumnSpec,
    },
    DeleteColumn {
        table: &'a TableSpec,
        column: &'a ColumnSpec,
    },
    SetColumnDefaultValue {
        table: &'a TableSpec,
        column: &'a ColumnSpec,
        default_value: String,
    },
    UnsetColumnDefaultValue {
        table: &'a TableSpec,
        column: &'a ColumnSpec,
    },

    CreateExtension {
        extension: String,
    },
    RemoveExtension {
        extension: String,
    },

    CreateUniqueConstraint {
        table: &'a TableSpec,
        constraint_name: String,
        columns: HashSet<String>,
    },
    RemoveUniqueConstraint {
        table: &'a TableSpec,
        constraint: String,
    },

    SetNotNull {
        table: &'a TableSpec,
        column: &'a ColumnSpec,
    },
    UnsetNotNull {
        table: &'a TableSpec,
        column: &'a ColumnSpec,
    },
}

impl SchemaOp<'_> {
    pub fn to_sql(&self) -> SchemaStatement {
        fn create_index(column: &ColumnSpec, table_name: &str, post_statements: &mut Vec<String>) {
            // create indices for all columns except pk columns
            if !column.is_pk {
                post_statements.push(format!(
                    r#"CREATE INDEX ON "{}" ("{}");"#,
                    table_name, column.name
                ))
            }
        }

        match self {
            SchemaOp::CreateTable { table } => {
                let mut table_creation = table.creation_sql();

                for column in table.columns.iter() {
                    create_index(column, &table.name, &mut table_creation.post_statements)
                }

                table_creation
            }
            SchemaOp::DeleteTable { table } => table.deletion_sql(),
            SchemaOp::CreateColumn { table, column } => {
                let mut column_stmt = column.to_sql(&table.name);

                create_index(column, &table.name, &mut column_stmt.post_statements);

                SchemaStatement {
                    statement: format!(
                        "ALTER TABLE \"{}\" ADD {};",
                        table.name, column_stmt.statement
                    ),
                    pre_statements: column_stmt.pre_statements,
                    post_statements: column_stmt.post_statements,
                }
            }
            SchemaOp::DeleteColumn { table, column } => SchemaStatement {
                statement: format!(
                    "ALTER TABLE \"{}\" DROP COLUMN \"{}\";",
                    table.name, column.name
                ),
                ..Default::default()
            },
            SchemaOp::SetColumnDefaultValue {
                table,
                column,
                default_value,
            } => SchemaStatement {
                statement: format!(
                    "ALTER TABLE \"{}\" ALTER COLUMN \"{}\" SET DEFAULT {};",
                    table.name, column.name, default_value
                ),
                ..Default::default()
            },
            SchemaOp::UnsetColumnDefaultValue { table, column } => SchemaStatement {
                statement: format!(
                    "ALTER TABLE \"{}\" ALTER COLUMN \"{}\" DROP DEFAULT;",
                    table.name, column.name
                ),
                ..Default::default()
            },
            SchemaOp::CreateExtension { extension } => SchemaStatement {
                statement: format!("CREATE EXTENSION \"{extension}\";"),
                ..Default::default()
            },
            SchemaOp::RemoveExtension { extension } => SchemaStatement {
                statement: format!("DROP EXTENSION \"{extension}\";"),
                ..Default::default()
            },
            SchemaOp::CreateUniqueConstraint {
                table,
                constraint_name,
                columns,
            } => SchemaStatement {
                statement: format!(
                    "ALTER TABLE \"{}\" ADD CONSTRAINT \"{}\" UNIQUE ({});",
                    table.name,
                    constraint_name,
                    sorted_comma_list(columns, true)
                ),
                ..Default::default()
            },
            SchemaOp::RemoveUniqueConstraint { table, constraint } => SchemaStatement {
                statement: format!(
                    "ALTER TABLE \"{}\" DROP CONSTRAINT \"{}\";",
                    table.name, constraint
                ),
                ..Default::default()
            },
            SchemaOp::SetNotNull { table, column } => SchemaStatement {
                statement: format!(
                    "ALTER TABLE \"{}\" ALTER COLUMN \"{}\" SET NOT NULL;",
                    table.name, column.name,
                ),
                ..Default::default()
            },
            SchemaOp::UnsetNotNull { table, column } => SchemaStatement {
                statement: format!(
                    "ALTER TABLE \"{}\" ALTER COLUMN \"{}\" DROP NOT NULL;",
                    table.name, column.name
                ),
                ..Default::default()
            },
        }
    }

    pub fn error_string(&self) -> Option<String> {
        match self {
            SchemaOp::CreateTable { table } => Some(format!("The table `{}` exists in the model, but does not exist in the database.", table.name)),
            SchemaOp::DeleteTable { .. } => None, // An extra table in the database is not a problem

            SchemaOp::CreateColumn { table, column } => Some(format!("The column `{}` in the table `{}` exists in the model, but does not exist in the database table.", column.name, table.name)),
            SchemaOp::DeleteColumn { table, column } => {
                if column.is_nullable {
                    // Extra nullable columns are not a problem
                    None
                } else {
                    // Such column will cause failure when inserting new records
                    Some(format!("The non-nullable column `{}` in the table `{}` exists in the database table, but does not exist in the model.", 
                    column.name, table.name))
                }
            }

            SchemaOp::SetColumnDefaultValue { table, column, default_value } => Some(format!("The default value for column `{}` in table `{}` does not match `{}`", column.name, table.name, default_value)),
            SchemaOp::UnsetColumnDefaultValue { table, column } => Some(format!("The column `{}` in table `{}` is not set in the model.", column.name, table.name)),

            SchemaOp::CreateExtension { extension } => Some(format!("The model requires the extension `{extension}`.")),
            SchemaOp::RemoveExtension { .. } => None,

            SchemaOp::CreateUniqueConstraint { table, columns, constraint_name } => Some(format!("The model requires a unique constraint named `{}` for the following columns in table `{}`: {}", constraint_name, table.name, sorted_comma_list(columns, false))),
            SchemaOp::RemoveUniqueConstraint { table, constraint } => {
                // Extra unqiueness constraint may make inserts fail even if model allows it
                Some(format!("Extra unique constaint `{}` in table `{}` found that is not require by the model.", constraint, table.name))
            }

            SchemaOp::SetNotNull { table, column } => Some(format!("The model requires that the column `{}` in table `{}` is not nullable. All records in the database must have a non-null value for this column before migration.", column.name, table.name)),
            SchemaOp::UnsetNotNull { table, column } => Some(format!("The model requires that the column `{}` in table `{}` is nullable.", column.name, table.name)),
        }
    }
}
