use std::{path::PathBuf, time::SystemTime};

use super::command::Command;
use anyhow::Result;
use payas_model::spec::FromModel;
use payas_sql::{
    schema::{op::SchemaOp, spec::SchemaSpec},
    Database, PhysicalColumnType, PhysicalTable,
};

/// Perform a database migration for a claytip model
pub struct MigrateCommand {
    pub model: PathBuf,
    pub comment_destructive_changes: bool,
}

pub fn migration_statements(
    old_schema_spec: SchemaSpec,
    new_schema_spec: SchemaSpec,
) -> Vec<(String, bool)> {
    let mut pre_statements = vec![];
    let mut statements = vec![];
    let mut post_statements = vec![];

    let diffs = diff_schema(&old_schema_spec, &new_schema_spec);

    for diff in diffs.iter() {
        let is_destructive = match diff {
            SchemaOp::DeleteColumn { .. }
            | SchemaOp::DeleteTable { .. }
            | SchemaOp::RemoveExtension { .. } => true,

            SchemaOp::CreateColumn { .. }
            | SchemaOp::CreateTable { .. }
            | SchemaOp::CreateExtension { .. } => false,
        };

        let statement = diff.to_sql();

        for constraint in statement.pre_statements.into_iter() {
            pre_statements.push((constraint, is_destructive));
        }

        statements.push((statement.statement, is_destructive));

        for constraint in statement.post_statements.into_iter() {
            post_statements.push((constraint, is_destructive));
        }
    }

    pre_statements.extend(statements);
    pre_statements.extend(post_statements);
    pre_statements
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

            let statements = migration_statements(old_schema.value, new_schema);

            for (statement, is_destructive) in statements {
                if is_destructive && self.comment_destructive_changes {
                    print!("-- ");
                }
                println!("{}", statement);
            }

            Ok(())
        })
    }
}

fn diff_schema<'a>(old: &'a SchemaSpec, new: &'a SchemaSpec) -> Vec<SchemaOp<'a>> {
    let existing_tables = &old.table_specs;
    let new_tables = &new.table_specs;
    let mut changes = vec![];

    // extension removal
    let extensions_to_drop = old.required_extensions.difference(&new.required_extensions);
    for extension in extensions_to_drop {
        changes.push(SchemaOp::RemoveExtension {
            extension: extension.clone(),
        })
    }

    // extension creation
    let extensions_to_create = new.required_extensions.difference(&old.required_extensions);
    for extension in extensions_to_create {
        changes.push(SchemaOp::CreateExtension {
            extension: extension.clone(),
        })
    }

    for old_table in old.table_specs.iter() {
        // try to find a table with the same name in the new spec
        match new_tables
            .iter()
            .find(|new_table| old_table.name == new_table.name)
        {
            // table exists, compare columns
            Some(new_table) => changes.extend(diff_table(old_table, new_table)),

            // table does not exist, deletion
            None => changes.push(SchemaOp::DeleteTable { table: old_table }),
        }
    }

    // try to find a table that needs to be created
    for new_table in new.table_specs.iter() {
        if !existing_tables
            .iter()
            .any(|old_table| new_table.name == old_table.name)
        {
            // new table
            changes.push(SchemaOp::CreateTable { table: new_table })
        }
    }

    changes
}

fn diff_table<'a>(old: &'a PhysicalTable, new: &'a PhysicalTable) -> Vec<SchemaOp<'a>> {
    let existing_columns = &old.columns;
    let new_columns = &new.columns;
    let mut changes = vec![];

    for column in old.columns.iter() {
        match column.typ {
            PhysicalColumnType::ColumnReference { .. } => {}
            _ => {
                if !new_columns.contains(column) {
                    // column deletion
                    changes.push(SchemaOp::DeleteColumn { table: new, column });
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
                    changes.push(SchemaOp::CreateColumn { table: new, column });
                }
            }
        }
    }

    changes
}
