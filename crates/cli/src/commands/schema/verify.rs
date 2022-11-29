//! Subcommands under the `schema` subcommand

use anyhow::{anyhow, Result};
use builder::error::ParserError;
use payas_sql::{
    database_error::DatabaseError,
    schema::{op::SchemaOp, spec::SchemaSpec},
};
use std::{
    fmt::Display,
    path::{Path, PathBuf},
    time::SystemTime,
};

use crate::{commands::command::Command, util::open_database};

use super::util;

/// Verify that a schema is compatible with a claytip model
pub struct VerifyCommand {
    pub model: PathBuf,
    pub database: Option<String>,
}

impl Command for VerifyCommand {
    fn run(&self, _system_start_time: Option<SystemTime>) -> Result<()> {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_io()
            .build()
            .unwrap();

        let verification_result = rt.block_on(verify(&self.model, self.database.as_deref()));

        match &verification_result {
            Ok(()) => eprintln!("This model is compatible with the database schema!"),
            Err(e @ VerificationErrors::ModelNotCompatible(_)) => {
                eprintln!("This model is not compatible with the current database schema. You may need to update your model to match, or perform a migration to update it.");
                eprintln!("The following issues should be corrected:");
                eprintln!("{}", e)
            }
            Err(e) => eprintln!("Error: {}", e),
        }

        verification_result.map_err(|_| anyhow!("Incompatible model."))
    }
}

pub enum VerificationErrors {
    PostgresError(DatabaseError),
    ParserError(ParserError),
    ModelNotCompatible(Vec<String>),
}

impl Display for VerificationErrors {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VerificationErrors::PostgresError(e) => write!(f, "Postgres error: {}", e),
            VerificationErrors::ParserError(e) => write!(f, "Error while parsing model: {}", e),
            VerificationErrors::ModelNotCompatible(e) => {
                for error in e.iter() {
                    writeln!(f, "- {}", error)?
                }

                Ok(())
            }
        }
    }
}

pub async fn verify(model: &Path, database: Option<&str>) -> Result<(), VerificationErrors> {
    let database = open_database(database).map_err(VerificationErrors::PostgresError)?;
    let client = database
        .get_client()
        .await
        .map_err(VerificationErrors::PostgresError)?;

    // import schema from db
    let db_schema = SchemaSpec::from_db(&client)
        .await
        .map_err(VerificationErrors::PostgresError)?;
    for issue in &db_schema.issues {
        println!("{}", issue);
    }

    // parse provided model
    let postgres_subsystem =
        util::create_postgres_system(model).map_err(VerificationErrors::ParserError)?;
    let model_schema = SchemaSpec::from_model(postgres_subsystem.tables.into_iter().collect());

    // generate diff
    let migration = db_schema.value.diff(&model_schema);

    let mut errors = vec![];

    for op in migration.iter() {
        match op {
                    SchemaOp::CreateTable { table } => errors.push(format!("The table `{}` exists in the model, but does not exist in the database.", table.name)),
                    SchemaOp::CreateColumn { column } => errors.push(format!("The column `{}` in the table `{}` exists in the model, but does not exist in the database table.", column.column_name, column.table_name)),
                    SchemaOp::SetColumnDefaultValue { column, default_value } => errors.push(format!("The default value for column `{}` in table `{}` does not match `{}`", column.column_name, column.table_name, default_value)),
                    SchemaOp::UnsetColumnDefaultValue { column } => errors.push(format!("The column `{}` in table `{}` is not set in the model.", column.column_name, column.table_name)),
                    SchemaOp::CreateExtension { extension } => errors.push(format!("The model requires the extension `{}`.", extension)),
                    SchemaOp::CreateUniqueConstraint { table, columns, constraint_name } => errors.push(format!("The model requires a unique constraint named `{}` for the following columns in table `{}`: {}", constraint_name, table.name, columns.join(", "))),
                    SchemaOp::SetNotNull { column } => errors.push(format!("The model requires that the column `{}` in table `{}` is not nullable. All records in the database must have a non-null value for this column before migration.", column.column_name, column.table_name)),
                    _ => {}
                }
    }

    if !errors.is_empty() {
        Err(VerificationErrors::ModelNotCompatible(errors))
    } else {
        Ok(())
    }
}
