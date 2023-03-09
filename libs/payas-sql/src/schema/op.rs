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
        columns: Vec<String>,
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
    pub fn to_sql(&self) -> SchemaStatement {
        fn create_index(column: &PhysicalColumn, post_statements: &mut Vec<String>) {
            // create indices for all columns except pk columns
            if !column.is_pk {
                post_statements.push(format!(
                    "CREATE INDEX ON \"{}\" ({});",
                    column.table_name, column.name
                ))
            }
        }

        match self {
            SchemaOp::CreateTable { table } => {
                let mut table_creation = table.creation_sql();

                for column in table.columns.iter() {
                    create_index(column, &mut table_creation.post_statements)
                }

                table_creation
            }
            SchemaOp::DeleteTable { table } => table.deletion_sql(),
            SchemaOp::CreateColumn { column } => {
                let mut column_stmt = column.to_sql();

                create_index(column, &mut column_stmt.post_statements);

                SchemaStatement {
                    statement: format!(
                        "ALTER TABLE \"{}\" ADD {};",
                        &column.table_name, column_stmt.statement
                    ),
                    pre_statements: column_stmt.pre_statements,
                    post_statements: column_stmt.post_statements,
                }
            }
            SchemaOp::DeleteColumn { column } => SchemaStatement {
                statement: format!(
                    "ALTER TABLE \"{}\" DROP COLUMN \"{}\";",
                    &column.table_name, column.name
                ),
                ..Default::default()
            },
            SchemaOp::SetColumnDefaultValue {
                column,
                default_value,
            } => SchemaStatement {
                statement: format!(
                    "ALTER TABLE \"{}\" ALTER COLUMN \"{}\" SET DEFAULT {};",
                    column.table_name, column.name, default_value
                ),
                ..Default::default()
            },
            SchemaOp::UnsetColumnDefaultValue { column } => SchemaStatement {
                statement: format!(
                    "ALTER TABLE \"{}\" ALTER COLUMN \"{}\" DROP DEFAULT;",
                    column.table_name, column.name
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
                    columns.join(", ")
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
                    column.table_name, column.name,
                ),
                ..Default::default()
            },
            SchemaOp::UnsetNotNull { column } => SchemaStatement {
                statement: format!(
                    "ALTER TABLE \"{}\" ALTER COLUMN \"{}\" DROP NOT NULL;",
                    column.table_name, column.name
                ),
                ..Default::default()
            },
        }
    }
}
