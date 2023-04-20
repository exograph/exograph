// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use anyhow::{anyhow, Result};
use builder::error::ParserError;
use clap::Command;
use exo_sql::{database_error::DatabaseError, schema::spec::SchemaSpec};
use std::{
    fmt::Display,
    path::{Path, PathBuf},
};

use crate::{
    commands::command::{database_arg, get, get_required, model_file_arg, CommandDefinition},
    util::open_database,
};

use super::util;

pub(super) struct VerifyCommandDefinition {}

impl CommandDefinition for VerifyCommandDefinition {
    fn command(&self) -> clap::Command {
        Command::new("verify")
            .about("Verify that the database schema is compatible with a Exograph model")
            .arg(model_file_arg())
            .arg(database_arg())
    }

    /// Verify that a schema is compatible with a exograph model

    fn execute(&self, matches: &clap::ArgMatches) -> Result<()> {
        let model: PathBuf = get_required(matches, "model")?;
        let database: Option<String> = get(matches, "database");

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_io()
            .build()
            .unwrap();

        let verification_result = rt.block_on(verify(&model, database.as_deref()));

        match &verification_result {
            Ok(()) => eprintln!("This model is compatible with the database schema!"),
            Err(e @ VerificationErrors::ModelNotCompatible(_)) => {
                eprintln!("This model is not compatible with the current database schema. You may need to update your model to match, or perform a migration to update it.");
                eprintln!("The following issues should be corrected:");
                eprintln!("{e}")
            }
            Err(e) => eprintln!("Error: {e}"),
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
            VerificationErrors::PostgresError(e) => write!(f, "Postgres error: {e}"),
            VerificationErrors::ParserError(e) => write!(f, "Error while parsing model: {e}"),
            VerificationErrors::ModelNotCompatible(e) => {
                for error in e.iter() {
                    writeln!(f, "- {error}")?
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
        println!("{issue}");
    }

    // parse provided model
    let postgres_subsystem =
        util::create_postgres_system(model).map_err(VerificationErrors::ParserError)?;
    let model_schema = SchemaSpec::from_model(postgres_subsystem.tables.into_iter().collect());

    // generate diff
    let migration = db_schema.value.diff(&model_schema);

    let mut errors = vec![];

    for op in migration.iter() {
        if let Some(error) = op.error_string() {
            errors.push(error);
        }
    }

    if !errors.is_empty() {
        Err(VerificationErrors::ModelNotCompatible(errors))
    } else {
        Ok(())
    }
}
