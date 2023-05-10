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
use exo_sql::database_error::DatabaseError;
use std::{fmt::Display, path::PathBuf};

use crate::commands::command::{
    database_arg, default_model_file, ensure_exo_project_dir, get, CommandDefinition,
};

use super::migration::Migration;

pub(super) struct VerifyCommandDefinition {}

impl CommandDefinition for VerifyCommandDefinition {
    fn command(&self) -> clap::Command {
        Command::new("verify")
            .about("Verify that the database schema is compatible with a Exograph model")
            .arg(database_arg())
    }

    /// Verify that a schema is compatible with a exograph model

    fn execute(&self, matches: &clap::ArgMatches) -> Result<()> {
        ensure_exo_project_dir(&PathBuf::from("."))?;

        let model: PathBuf = default_model_file();
        let database: Option<String> = get(matches, "database");

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_io()
            .build()
            .unwrap();

        let verification_result = rt.block_on(Migration::verify(database.as_deref(), &model));

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

impl From<DatabaseError> for VerificationErrors {
    fn from(e: DatabaseError) -> Self {
        VerificationErrors::PostgresError(e)
    }
}

impl From<ParserError> for VerificationErrors {
    fn from(e: ParserError) -> Self {
        VerificationErrors::ParserError(e)
    }
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
