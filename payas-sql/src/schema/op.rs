use crate::{PhysicalColumn, PhysicalTable};

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

    CreateConstraint {
        table: &'a PhysicalTable,
        constraint_name: String,
        columns: Vec<String>,
    },
    RemoveConstraint {
        table: &'a PhysicalTable,
        constraint: String,
    },
}

impl SchemaOp<'_> {
    pub fn to_sql(&self) -> SchemaStatement {
        match self {
            SchemaOp::CreateTable { table } => table.creation_sql(),
            SchemaOp::DeleteTable { table } => table.deletion_sql(),
            SchemaOp::CreateColumn { table, column } => {
                let column = column.to_sql(&table.name);

                SchemaStatement {
                    statement: format!("ALTER TABLE \"{}\" ADD {};", table.name, column.statement),
                    pre_statements: column.pre_statements,
                    post_statements: column.post_statements,
                }
            }
            SchemaOp::DeleteColumn { table, column } => SchemaStatement {
                statement: format!(
                    "ALTER TABLE \"{}\" DROP COLUMN \"{}\";",
                    table.name, column.column_name
                ),
                ..Default::default()
            },
            SchemaOp::CreateExtension { extension } => SchemaStatement {
                statement: format!("CREATE EXTENSION \"{}\";", extension),
                ..Default::default()
            },
            SchemaOp::RemoveExtension { extension } => SchemaStatement {
                statement: format!("DROP EXTENSION \"{}\";", extension),
                ..Default::default()
            },
            SchemaOp::CreateConstraint {
                table,
                constraint_name,
                columns,
            } => SchemaStatement {
                statement: format!(
                    "ALTER TABLE \"{}\" ADD CONSTRAINT \"{}\" UNIQUE ({});",
                    table.name,
                    constraint_name,
                    columns.join(", ")
                ),
                ..Default::default()
            },
            SchemaOp::RemoveConstraint { table, constraint } => SchemaStatement {
                statement: format!(
                    "ALTER TABLE \"{}\" DROP CONSTRAINT \"{}\";",
                    table.name, constraint
                ),
                ..Default::default()
            },
        }
    }
}
