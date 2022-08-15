//! Subcommands under the `schema` subcommand

use anyhow::{anyhow, Result};
use payas_sql::{
    schema::{op::SchemaOp, spec::SchemaSpec},
    Database,
};
use std::{path::PathBuf, time::SystemTime};

use crate::commands::command::Command;

/// Verify that a schema is compatible with a claytip model
pub struct VerifyCommand {
    pub model: PathBuf,
    pub database: String,
}

impl Command for VerifyCommand {
    fn run(&self, _system_start_time: Option<SystemTime>) -> Result<()> {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_io()
            .build()
            .unwrap();

        rt.block_on(async {
            let database = Database::from_db_url(&self.database)?; // TODO: error handling here
            let client = database.get_client().await?;

            // import schema from db
            let db_schema = SchemaSpec::from_db(&client).await?;
            for issue in &db_schema.issues {
                println!("{}", issue);
            }

            // parse provided model
            let model_system = payas_parser::build_system(&self.model)?;
            let model_schema = SchemaSpec::from_model(model_system.tables.into_iter().collect());

            // generate diff
            let migration = db_schema.value.diff(&model_schema);

            let mut is_compatible = true;

            for op in migration.iter() {
                let mut pass = false;
                match op {
                    SchemaOp::CreateTable { table } => println!("The table `{}` exists in the model, but does not exist in the database.", table.name),
                    SchemaOp::CreateColumn { column } => println!("The column `{}` in the table `{}` exists in the model, but does not exist in the database table.", column.column_name, column.table_name),
                    SchemaOp::SetColumnDefaultValue { column, default_value } => println!("The default value for column `{}` in table `{}` does not match `{}`", column.column_name, column.table_name, default_value),
                    SchemaOp::UnsetColumnDefaultValue { column } => println!("The column `{}` in table `{}` is not set in the model.", column.column_name, column.table_name),
                    SchemaOp::CreateExtension { extension } => println!("The model requires the extension `{}`.", extension),
                    SchemaOp::CreateUniqueConstraint { table, columns, constraint_name } => println!("The model requires a unique constraint named `{}` for the following columns in table `{}`: {}", constraint_name, table.name, columns.join(", ")),
                    SchemaOp::SetNotNull { column } => println!("The model requires that the column `{}` in table `{}` is not nullable.", column.column_name, column.table_name),
                    _ => {
                        pass = true;
                    }
                }

                is_compatible &= pass;
            }

            if !is_compatible {
                Err(anyhow!("=== This model is not compatible with the database schema! ==="))
            } else {
                println!("This model is compatible with the database schema.");
                Ok(())
            }

        })
    }
}
