// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::collections::HashSet;

use crate::{schema::constraint::sorted_comma_list, Database, PhysicalColumn, PhysicalTable};

use super::statement::SchemaStatement;

/// An execution unit of SQL, representing an operation that can create or destroy resources.
#[derive(Debug)]
pub enum SchemaOp<'a> {
    CreateTable {
        table: &'a PhysicalTable,
    },
    DeleteTable {
        table: &'a PhysicalTable,
    },

    CreateColumn {
        column: &'a PhysicalColumn,
    },
    DeleteColumn {
        column: &'a PhysicalColumn,
    },
    SetColumnDefaultValue {
        column: &'a PhysicalColumn,
        default_value: String,
    },
    UnsetColumnDefaultValue {
        column: &'a PhysicalColumn,
    },

    CreateExtension {
        extension: String,
    },
    RemoveExtension {
        extension: String,
    },

    CreateUniqueConstraint {
        table: &'a PhysicalTable,
        constraint_name: String,
        columns: HashSet<String>,
    },
    RemoveUniqueConstraint {
        table: &'a PhysicalTable,
        constraint: String,
    },

    SetNotNull {
        column: &'a PhysicalColumn,
    },
    UnsetNotNull {
        column: &'a PhysicalColumn,
    },
}

impl SchemaOp<'_> {
    pub fn to_sql(&self, old_database: &Database, new_database: &Database) -> SchemaStatement {
        fn create_index(
            column: &PhysicalColumn,
            post_statements: &mut Vec<String>,
            database: &Database,
        ) {
            // create indices for all columns except pk columns
            if !column.is_pk {
                post_statements.push(format!(
                    r#"CREATE INDEX ON "{}" ("{}");"#,
                    database.get_table(column.table_id).name,
                    column.name
                ))
            }
        }

        match self {
            SchemaOp::CreateTable { table } => {
                let mut table_creation = table.creation_sql(new_database);

                for column in table.columns.iter() {
                    create_index(column, &mut table_creation.post_statements, new_database)
                }

                table_creation
            }
            SchemaOp::DeleteTable { table } => table.deletion_sql(),
            SchemaOp::CreateColumn { column } => {
                let mut column_stmt = column.to_sql(new_database);

                create_index(column, &mut column_stmt.post_statements, new_database);

                SchemaStatement {
                    statement: format!(
                        "ALTER TABLE \"{}\" ADD {};",
                        &new_database.get_table(column.table_id).name,
                        column_stmt.statement
                    ),
                    pre_statements: column_stmt.pre_statements,
                    post_statements: column_stmt.post_statements,
                }
            }
            SchemaOp::DeleteColumn { column } => SchemaStatement {
                statement: format!(
                    "ALTER TABLE \"{}\" DROP COLUMN \"{}\";",
                    old_database.get_table(column.table_id).name,
                    column.name
                ),
                ..Default::default()
            },
            SchemaOp::SetColumnDefaultValue {
                column,
                default_value,
            } => SchemaStatement {
                statement: format!(
                    "ALTER TABLE \"{}\" ALTER COLUMN \"{}\" SET DEFAULT {};",
                    new_database.get_table(column.table_id).name,
                    column.name,
                    default_value
                ),
                ..Default::default()
            },
            SchemaOp::UnsetColumnDefaultValue { column } => SchemaStatement {
                statement: format!(
                    "ALTER TABLE \"{}\" ALTER COLUMN \"{}\" DROP DEFAULT;",
                    new_database.get_table(column.table_id).name,
                    column.name
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
            SchemaOp::SetNotNull { column } => SchemaStatement {
                statement: format!(
                    "ALTER TABLE \"{}\" ALTER COLUMN \"{}\" SET NOT NULL;",
                    new_database.get_table(column.table_id).name,
                    column.name,
                ),
                ..Default::default()
            },
            SchemaOp::UnsetNotNull { column } => SchemaStatement {
                statement: format!(
                    "ALTER TABLE \"{}\" ALTER COLUMN \"{}\" DROP NOT NULL;",
                    new_database.get_table(column.table_id).name,
                    column.name
                ),
                ..Default::default()
            },
        }
    }

    pub fn error_string(&self, old_database: &Database, new_database: &Database) -> Option<String> {
        match self {
            SchemaOp::CreateTable { table } => Some(format!("The table `{}` exists in the model, but does not exist in the database.", table.name)),
            SchemaOp::DeleteTable { .. } => None, // An extra table in the database is not a problem

            SchemaOp::CreateColumn { column } => Some(format!("The column `{}` in the table `{}` exists in the model, but does not exist in the database table.", column.name, column.get_table_name(new_database))),
            SchemaOp::DeleteColumn { column } => {
                if column.is_nullable {
                    // Extra nullable columns are not a problem
                    None
                } else {
                    // Such column will cause failure when inserting new records
                    Some(format!("The non-nullable column `{}` in the table `{}` exists in the database table, but does not exist in the model.", 
                    column.name, column.get_table_name(old_database)))
                }
            }

            SchemaOp::SetColumnDefaultValue { column, default_value } => Some(format!("The default value for column `{}` in table `{}` does not match `{}`", column.name, column.get_table_name(new_database), default_value)),
            SchemaOp::UnsetColumnDefaultValue { column } => Some(format!("The column `{}` in table `{}` is not set in the model.", column.name, column.get_table_name(new_database))),

            SchemaOp::CreateExtension { extension } => Some(format!("The model requires the extension `{extension}`.")),
            SchemaOp::RemoveExtension { .. } => None,

            SchemaOp::CreateUniqueConstraint { table, columns, constraint_name } => Some(format!("The model requires a unique constraint named `{}` for the following columns in table `{}`: {}", constraint_name, table.name, sorted_comma_list(columns, false))),
            SchemaOp::RemoveUniqueConstraint { table, constraint } => {
                // Extra unqiueness constraint may make inserts fail even if model allows it
                Some(format!("Extra unique constaint `{}` in table `{}` found that is not require by the model.", constraint, table.name))
            }

            SchemaOp::SetNotNull { column } => Some(format!("The model requires that the column `{}` in table `{}` is not nullable. All records in the database must have a non-null value for this column before migration.", column.name, column.get_table_name(new_database))),
            SchemaOp::UnsetNotNull { column } => Some(format!("The model requires that the column `{}` in table `{}` is nullable.", column.name, column.get_table_name(new_database))),
        }
    }
}
