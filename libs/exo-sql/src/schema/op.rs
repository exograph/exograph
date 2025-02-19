// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::collections::HashSet;

use crate::{
    schema::{constraint::sorted_comma_list, index_spec::IndexSpec},
    SchemaObjectName,
};

use super::{
    column_spec::{ColumnReferenceSpec, ColumnSpec},
    enum_spec::EnumSpec,
    function_spec::FunctionSpec,
    statement::SchemaStatement,
    table_spec::TableSpec,
    trigger_spec::TriggerSpec,
};

/// An execution unit of SQL, representing an operation that can create or destroy resources.
#[derive(Debug)]
pub enum SchemaOp<'a> {
    CreateSchema {
        schema: String,
    },
    DeleteSchema {
        schema: String,
    },
    RenameSchema {
        old_name: String,
        new_name: String,
    },

    CreateSequence {
        sequence: SchemaObjectName,
    },
    DeleteSequence {
        sequence: SchemaObjectName,
    },

    CreateTable {
        table: &'a TableSpec,
    },
    DeleteTable {
        table: &'a TableSpec,
    },
    RenameTable {
        table: &'a TableSpec,
        new_name: SchemaObjectName,
    },

    CreateEnum {
        enum_: &'a EnumSpec,
    },
    DeleteEnum {
        enum_: &'a EnumSpec,
    },

    CreateColumn {
        table: &'a TableSpec,
        column: &'a ColumnSpec,
    },
    DeleteColumn {
        table: &'a TableSpec,
        column: &'a ColumnSpec,
    },
    RenameColumn {
        table: &'a TableSpec,
        name: String,
        new_name: String,
    },
    CreateIndex {
        table: &'a TableSpec,
        index: &'a IndexSpec,
    },
    DeleteIndex {
        table: &'a TableSpec,
        index: &'a IndexSpec,
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

    CreateForeignKeyReference {
        table: &'a TableSpec,
        name: String,
        reference_columns: Vec<(&'a ColumnSpec, &'a ColumnReferenceSpec)>,
    },
    DeleteForeignKeyReference {
        table: &'a TableSpec,
        name: String,
    },

    SetNotNull {
        table: &'a TableSpec,
        column: &'a ColumnSpec,
    },
    UnsetNotNull {
        table: &'a TableSpec,
        column: &'a ColumnSpec,
    },

    CreateFunction {
        function: &'a FunctionSpec,
    },
    DeleteFunction {
        name: &'a str,
    },
    CreateOrReplaceFunction {
        function: &'a FunctionSpec,
    },

    CreateTrigger {
        trigger: &'a TriggerSpec,
        table_name: &'a SchemaObjectName,
    },
    DeleteTrigger {
        trigger: &'a TriggerSpec,
        table_name: &'a SchemaObjectName,
    },
}

impl SchemaOp<'_> {
    pub fn to_sql(&self) -> SchemaStatement {
        match self {
            SchemaOp::CreateSchema { schema } => SchemaStatement {
                statement: format!("CREATE SCHEMA IF NOT EXISTS \"{schema}\";"),
                ..Default::default()
            },
            SchemaOp::DeleteSchema { schema } => SchemaStatement {
                statement: format!("DROP SCHEMA IF EXISTS \"{schema}\" CASCADE;"),
                ..Default::default()
            },
            SchemaOp::RenameSchema { old_name, new_name } => SchemaStatement {
                statement: format!("ALTER SCHEMA \"{old_name}\" RENAME TO \"{new_name}\";"),
                ..Default::default()
            },

            SchemaOp::CreateSequence { sequence } => SchemaStatement {
                statement: format!("CREATE SEQUENCE IF NOT EXISTS \"{}\";", sequence.name),
                ..Default::default()
            },
            SchemaOp::DeleteSequence { sequence } => SchemaStatement {
                statement: format!("DROP SEQUENCE IF EXISTS \"{}\";", sequence.name),
                ..Default::default()
            },

            SchemaOp::CreateTable { table } => table.creation_sql(),
            SchemaOp::DeleteTable { table } => table.deletion_sql(),
            SchemaOp::RenameTable { table, new_name } => SchemaStatement {
                statement: format!(
                    "ALTER TABLE {} RENAME TO {};",
                    table.sql_name(),
                    new_name.sql_name()
                ),
                ..Default::default()
            },

            SchemaOp::CreateEnum { enum_ } => enum_.creation_sql(),
            SchemaOp::DeleteEnum { enum_ } => enum_.deletion_sql(),

            SchemaOp::CreateColumn { table, column } => {
                let column_stmt = column.to_sql(table.has_single_pk());

                SchemaStatement {
                    statement: format!(
                        "ALTER TABLE {} ADD {};",
                        table.sql_name(),
                        column_stmt.statement
                    ),
                    pre_statements: column_stmt.pre_statements,
                    post_statements: column_stmt.post_statements,
                }
            }
            SchemaOp::DeleteColumn { table, column } => SchemaStatement {
                statement: format!(
                    "ALTER TABLE {} DROP COLUMN \"{}\";",
                    table.sql_name(),
                    column.name
                ),
                ..Default::default()
            },
            SchemaOp::RenameColumn {
                table,
                name,
                new_name,
            } => SchemaStatement {
                statement: format!(
                    "ALTER TABLE {} RENAME COLUMN \"{}\" TO \"{}\";",
                    table.sql_name(),
                    name,
                    new_name
                ),
                ..Default::default()
            },

            SchemaOp::CreateIndex { table, index } => SchemaStatement {
                statement: index.creation_sql(&table.name),
                ..Default::default()
            },
            SchemaOp::DeleteIndex { index, .. } => SchemaStatement {
                statement: format!("DROP INDEX \"{}\";", index.name),
                ..Default::default()
            },

            SchemaOp::SetColumnDefaultValue {
                table,
                column,
                default_value,
            } => SchemaStatement {
                statement: format!(
                    "ALTER TABLE {} ALTER COLUMN \"{}\" SET DEFAULT {};",
                    table.sql_name(),
                    column.name,
                    default_value
                ),
                ..Default::default()
            },
            SchemaOp::UnsetColumnDefaultValue { table, column } => SchemaStatement {
                statement: format!(
                    "ALTER TABLE {} ALTER COLUMN \"{}\" DROP DEFAULT;",
                    table.sql_name(),
                    column.name
                ),
                ..Default::default()
            },

            SchemaOp::CreateExtension { extension } => SchemaStatement {
                statement: format!("CREATE EXTENSION IF NOT EXISTS \"{extension}\";"),
                ..Default::default()
            },
            SchemaOp::RemoveExtension { extension } => SchemaStatement {
                statement: format!("DROP EXTENSION IF EXISTS \"{extension}\";"),
                ..Default::default()
            },

            SchemaOp::CreateUniqueConstraint {
                table,
                constraint_name,
                columns,
            } => SchemaStatement {
                statement: format!(
                    "ALTER TABLE {} ADD CONSTRAINT \"{}\" UNIQUE ({});",
                    table.sql_name(),
                    constraint_name,
                    sorted_comma_list(columns, true)
                ),
                ..Default::default()
            },
            SchemaOp::RemoveUniqueConstraint { table, constraint } => SchemaStatement {
                statement: format!(
                    "ALTER TABLE {} DROP CONSTRAINT IF EXISTS \"{}\";",
                    table.sql_name(),
                    constraint
                ),
                ..Default::default()
            },

            SchemaOp::CreateForeignKeyReference {
                table,
                name,
                reference_columns,
            } => {
                let mut reference_columns = reference_columns.clone();
                reference_columns
                    .sort_by(|(column1, _), (column2, _)| column1.name.cmp(&column2.name));

                let (self_columns, foreign_columns): (Vec<&ColumnSpec>, Vec<&ColumnReferenceSpec>) =
                    reference_columns.into_iter().unzip();

                let constraint_name = format!(
                    "{}_{}_fk",
                    table.name.fully_qualified_name_with_sep("_"),
                    name
                );

                let foreign_reference_columns = if foreign_columns.len() == 1 {
                    // If there is only one foreign column, we don't need to specify the columns in the foreign key constraint (assume it's the primary key)
                    // TODO: We keep this behavior for now to avoid changing all migration tests, but we should do that in a future PR
                    "".to_string()
                } else {
                    let names = foreign_columns
                        .iter()
                        .map(|column_reference| {
                            format!("\"{}\"", column_reference.foreign_pk_column_name)
                        })
                        .collect::<Vec<_>>()
                        .join(", ");

                    format!(" ({names})")
                };

                let foreign_constraint = format!(
                    r#"ALTER TABLE {} ADD CONSTRAINT "{constraint_name}" FOREIGN KEY ({}) REFERENCES {}{};"#,
                    table.name.sql_name(),
                    self_columns
                        .iter()
                        .map(|c| format!("\"{}\"", c.name.as_str()))
                        .collect::<Vec<_>>()
                        .join(", "),
                    foreign_columns[0].foreign_table_name.sql_name(), // Foreign columns all point to the same table, so use any of them
                    foreign_reference_columns
                );

                SchemaStatement {
                    post_statements: vec![foreign_constraint],
                    ..Default::default()
                }
            }
            SchemaOp::DeleteForeignKeyReference { table, name } => SchemaStatement {
                statement: format!(
                    "ALTER TABLE {} DROP CONSTRAINT \"{}\";",
                    table.sql_name(),
                    name
                ),
                ..Default::default()
            },

            SchemaOp::SetNotNull { table, column } => SchemaStatement {
                statement: format!(
                    "ALTER TABLE {} ALTER COLUMN \"{}\" SET NOT NULL;",
                    table.sql_name(),
                    column.name,
                ),
                ..Default::default()
            },
            SchemaOp::UnsetNotNull { table, column } => SchemaStatement {
                statement: format!(
                    "ALTER TABLE {} ALTER COLUMN \"{}\" DROP NOT NULL;",
                    table.sql_name(),
                    column.name
                ),
                ..Default::default()
            },

            SchemaOp::CreateFunction { function } => SchemaStatement {
                statement: function.creation_sql(false),
                ..Default::default()
            },
            SchemaOp::CreateOrReplaceFunction { function } => SchemaStatement {
                statement: function.creation_sql(true),
                ..Default::default()
            },
            SchemaOp::DeleteFunction { name } => SchemaStatement {
                statement: format!("DROP FUNCTION {name};"),
                ..Default::default()
            },

            SchemaOp::CreateTrigger {
                trigger,
                table_name,
            } => SchemaStatement {
                statement: trigger.creation_sql(table_name),
                ..Default::default()
            },
            SchemaOp::DeleteTrigger {
                trigger,
                table_name,
            } => SchemaStatement {
                statement: format!(
                    "DROP TRIGGER {name} on {table};",
                    name = trigger.name,
                    table = table_name.sql_name()
                ),
                ..Default::default()
            },
        }
    }

    pub fn error_string(&self) -> Option<String> {
        match self {
            SchemaOp::CreateSchema { schema } => Some(format!("The schema `{schema}` exists in the model, but does not exist in the database.")),
            SchemaOp::DeleteSchema { .. } => None, // An extra schema in the database is not a problem
            SchemaOp::RenameSchema { .. } => None,

            SchemaOp::CreateSequence { sequence } => Some(format!("The sequence `{}` exists in the model, but does not exist in the database.", sequence.name)),
            SchemaOp::DeleteSequence { .. } => None, // An extra sequence in the database is not a problem

            SchemaOp::CreateTable { table } => Some(format!("The table `{}` exists in the model, but does not exist in the database.", table.sql_name())),
            SchemaOp::DeleteTable { .. } => None, // An extra table in the database is not a problem
            SchemaOp::RenameTable { .. } => None,

            SchemaOp::CreateEnum { enum_ } => Some(format!("The enum `{}` exists in the model, but does not exist in the database.", enum_.sql_name())),
            SchemaOp::DeleteEnum { .. } => None, // An extra enum in the database is not a problem

            SchemaOp::CreateColumn { table, column } => Some(format!("The column `{}` in the table `{}` exists in the model, but does not exist in the database table.", column.name, table.sql_name())),
            SchemaOp::DeleteColumn { table, column } => {
                if column.is_nullable {
                    // Extra nullable columns are not a problem
                    None
                } else {
                    // Such column will cause failure when inserting new records
                    Some(format!("The non-nullable column `{}` in the table `{}` exists in the database table, but does not exist in the model.", 
                    column.name, table.sql_name()))
                }
            }
            SchemaOp::RenameColumn { .. } => None,
            SchemaOp::CreateIndex { table, index } => Some(format!("The index `{}` in the table `{}` exists in the model, but does not exist in the database table.", index.name, table.sql_name())),
            SchemaOp::DeleteIndex { .. } => None, // An extra index in the database is not a problem

            SchemaOp::SetColumnDefaultValue { table, column, default_value } => Some(format!("The default value for column `{}` in table `{}` does not match `{}`", column.name, table.sql_name(), default_value)),
            SchemaOp::UnsetColumnDefaultValue { table, column } => Some(format!("The column `{}` in table `{}` is not set in the model.", column.name, table.sql_name())),

            SchemaOp::CreateExtension { extension } => Some(format!("The model requires the extension `{extension}`.")),
            SchemaOp::RemoveExtension { .. } => None,

            SchemaOp::CreateUniqueConstraint { table, columns, constraint_name } => {
                Some(format!("The model requires a unique constraint named `{}` for the following columns in table `{}`: {}", constraint_name, table.sql_name(), sorted_comma_list(columns, false)))
            },
            SchemaOp::RemoveUniqueConstraint { table, constraint } => {
                // Extra uniqueness constraint may make inserts fail even if model allows it
                Some(format!("Extra unique constaint `{}` in table `{}` found that is not require by the model.", constraint, table.sql_name()))
            }
            SchemaOp::CreateForeignKeyReference { table, reference_columns, .. } => {
                Some(format!("The model requires a foreign key constraint in table `{}` for the following columns: {}", table.sql_name(), reference_columns.iter().map(|(c, _)| c.name.as_str()).collect::<Vec<_>>().join(", ")))
            }
            SchemaOp::DeleteForeignKeyReference { table, name } => {
                Some(format!("Extra foreign key constraint `{}` in table `{}` found that is not require by the model.", name, table.sql_name()))
            }

            SchemaOp::SetNotNull { table, column } => {
                Some(format!("The model requires that the column `{}` in table `{}` is not nullable. All records in the database must have a non-null value for this column before migration.", column.name, table.sql_name()))
            },
            SchemaOp::UnsetNotNull { table, column } => Some(format!("The model requires that the column `{}` in table `{}` is nullable.", column.name, table.sql_name())),
            SchemaOp::CreateTrigger { trigger, table_name } => {
                Some(format!("The model requires a trigger named `{}` on table `{}`", trigger.name, table_name.sql_name()))
            },
            SchemaOp::DeleteTrigger { trigger, table_name } => {
                Some(format!("The trigger `{name}` on table `{table}` exists in the database, but does not exist in the model.", name = trigger.name, table = table_name.sql_name()))
            },
            SchemaOp::CreateFunction { function } => {
                Some(format!("The model requires a function named `{}`", function.name))
            },
            SchemaOp::DeleteFunction { name } => Some(format!("The function `{name}` exists in the database, but does not exist in the model.")),
            SchemaOp::CreateOrReplaceFunction { function } => {
                Some(format!("The model requires a function named `{}` with body `{}`", function.name, function.body))
            },
        }
    }
}
