// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use anyhow::{Result, anyhow};
use async_trait::async_trait;
use clap::Command;
use exo_env::Environment;
use exo_sql::TransactionMode;
use exo_sql::schema::migration::{Migration, VerificationErrors};
use std::path::PathBuf;
use std::sync::Arc;

use crate::commands::command::{
    CommandDefinition, database_arg, database_value, default_model_file, migration_scope_arg,
    migration_scope_value, yes_arg,
};
use crate::commands::util::{compute_migration_scope, use_ir_arg};
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
            .arg(migration_scope_arg())
            .arg(yes_arg())
    }

    /// Verify that a schema is compatible with a exograph model
    async fn execute(
        &self,
        matches: &clap::ArgMatches,
        _config: &Config,
        env: Arc<dyn Environment>,
    ) -> Result<()> {
        let model: PathBuf = default_model_file();
        let database_url = database_value(matches);
        let use_ir: bool = matches.get_flag("use-ir");
        let scope: Option<String> = migration_scope_value(matches);
        let db_client = util::open_database(
            database_url.as_deref(),
            TransactionMode::ReadOnly,
            env.as_ref(),
        )
        .await?;

        let database = util::extract_postgres_database(&model, None, use_ir).await?;
        let db_client = db_client.get_client().await?;
        let verification_result =
            Migration::verify(&db_client, &database, &compute_migration_scope(scope)).await;

        match &verification_result {
            Ok(()) => eprintln!("This model is compatible with the database schema!"),
            Err(e @ VerificationErrors::ModelNotCompatible(_)) => {
                eprintln!(
                    "This model is not compatible with the current database schema. You may need to update your model to match, or perform a migration to update it."
                );
                eprintln!("The following issues should be corrected:");
                eprintln!("{e}")
            }
            Err(e) => eprintln!("Error: {e}"),
        }

        verification_result.map_err(|_| anyhow!("Incompatible model."))
    }
}
