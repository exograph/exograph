use std::{path::PathBuf, time::SystemTime};

use super::command::Command;
use anyhow::Result;
use payas_model::spec::FromModel;
use payas_sql::{
    spec::{SQLOperation, SchemaSpec},
    Database, PhysicalColumnType, PhysicalTable,
};

/// Perform a database migration for a claytip model
pub struct MigrateCommand {
    pub model: PathBuf,
    pub comment_destructive_changes: bool,
}

impl Command for MigrateCommand {
    fn run(&self, _system_start_time: Option<SystemTime>) -> Result<()> {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_io()
            .build()
            .unwrap();

        rt.block_on(async {
            let database = Database::from_env(Some(1))?; // TODO: error handling here
            let client = database.get_client().await?;

            let old_schema = SchemaSpec::from_db(&client).await?;

            for issue in &old_schema.issues {
                println!("{}", issue);
            }

            let new_system = payas_parser::build_system(&self.model)?;
            let new_schema = SchemaSpec::from_model(new_system.tables);

            let diffs = diff_schema(&old_schema.value, &new_schema);

            for diff in diffs.iter() {
                match diff {
                    SQLOperation::DeleteColumn { .. }
                    | SQLOperation::DeleteTable { .. }
                    | SQLOperation::RemoveExtension { .. } => {
                        if self.comment_destructive_changes {
                            print!("-- ");
                        }
                    }

                    SQLOperation::CreateColumn { .. } | SQLOperation::CreateTable { .. } => {}
                };

                let statement = diff.to_sql();
                println!("{}", statement.statement);

                for constraint in statement.foreign_constraints_statements.iter() {
                    println!("{}", constraint);
                }
            }

            Ok(())
        })
    }
}

fn diff_schema<'a>(old: &'a SchemaSpec, new: &'a SchemaSpec) -> Vec<SQLOperation<'a>> {
    let existing_tables = &old.table_specs;
    let new_tables = &new.table_specs;
    let mut changes = vec![];

    for old_table in old.table_specs.iter() {
        match new_tables
            .iter()
            .find(|new_table| old_table.name == new_table.name)
        {
            Some(new_table) => {
                // table exists, compare columns
                changes.extend(diff_table(old_table, new_table))
            }

            None => {
                // table deletion
                changes.push(SQLOperation::DeleteTable { table: old_table })
            }
        }
    }

    for new_table in new.table_specs.iter() {
        if !existing_tables
            .iter()
            .any(|old_table| new_table.name == old_table.name)
        {
            // new table
            changes.push(SQLOperation::CreateTable { table: new_table })
        }
    }

    // extension addition is handled by schema.to_sql
    // noop

    // extension removal
    let dropped_extensions = old.required_extensions.difference(&new.required_extensions);
    for extension in dropped_extensions {
        changes.push(SQLOperation::RemoveExtension {
            extension: extension.clone(),
        })
    }

    changes
}

fn diff_table<'a>(old: &'a PhysicalTable, new: &'a PhysicalTable) -> Vec<SQLOperation<'a>> {
    let existing_columns = &old.columns;
    let new_columns = &new.columns;
    let mut changes = vec![];

    for column in old.columns.iter() {
        match column.typ {
            PhysicalColumnType::ColumnReference { .. } => {}
            _ => {
                if !new_columns.contains(column) {
                    // column deletion
                    changes.push(SQLOperation::DeleteColumn { table: new, column });
                }
            }
        }
    }

    for column in new.columns.iter() {
        match column.typ {
            PhysicalColumnType::ColumnReference { .. } => {}
            _ => {
                if !existing_columns.contains(column) {
                    // new column
                    changes.push(SQLOperation::CreateColumn { table: new, column });
                }
            }
        }
    }

    changes
}
