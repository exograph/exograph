// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use clap::Command;
use postgres_core_model::migration::{Migration, VerificationErrors};
use std::path::PathBuf;

use crate::commands::command::{database_arg, default_model_file, get, CommandDefinition};
use crate::commands::util::{migration_scope_from_env, use_ir_arg};
use crate::config::Config;

use super::util;

pub(super) struct VerifyCommandDefinition {}

#[async_trait]
impl CommandDefinition for VerifyCommandDefinition {
    fn command(&self) -> clap::Command {
        Command::new("verify")
            .about("Verify that the database schema is compatible with a Exograph model")
            .arg(database_arg())
            .arg(use_ir_arg())
    }

    /// Verify that a schema is compatible with a exograph model

    async fn execute(&self, matches: &clap::ArgMatches, _config: &Config) -> Result<()> {
        let model: PathBuf = default_model_file();
        let database: Option<String> = get(matches, "database");
        let use_ir: bool = matches.get_flag("use-ir");

        let db_client = util::open_database(database.as_deref()).await?;
        let database = util::extract_postgres_database(&model, None, use_ir).await?;
        let verification_result =
            Migration::verify(&db_client, &database, &migration_scope_from_env()).await;

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
