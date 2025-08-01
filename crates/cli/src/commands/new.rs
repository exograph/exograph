// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::fs::{File, create_dir_all};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Result, anyhow};
use async_trait::async_trait;
use clap::{Arg, ArgMatches, Command};
use colored::Colorize;
use common::env_processing::EnvProcessing;
use exo_env::Environment;
use exo_sql::TransactionMode;

use super::command::{
    CommandDefinition, database_arg, get_required, migration_scope_arg, mutation_access_arg,
    new_project_arg, query_access_arg,
};
use crate::commands::command::{
    database_value, migration_scope_value, mutation_access_value, query_access_value,
};
use crate::commands::schema::import::print_imported_tables;
use crate::config::Config;
use crate::schema::import::create_exo_model;
use crate::schema::util::open_database;

static SRC_INDEX_TEMPLATE: &[u8] = include_bytes!("templates/exo-new/src/index.exo");
static TESTS_TEST_TEMPLATE: &[u8] = include_bytes!("templates/exo-new/tests/basic-query.exotest");
static TESTS_INIT_TEMPLATE: &[u8] = include_bytes!("templates/exo-new/tests/init.gql");
static GITIGNORE_TEMPLATE: &[u8] = include_bytes!("templates/exo-new/gitignore");

pub struct NewCommandDefinition {}

#[async_trait]
impl CommandDefinition for NewCommandDefinition {
    fn command(&self) -> Command {
        Command::new("new")
            .about("Create a new Exograph project")
            .arg(new_project_arg())
            .arg(
                Arg::new("from-database")
                    .help("Create a new Exograph project from a database")
                    .long("from-database")
                    .required(false)
                    .num_args(0),
            )
            .arg(database_arg())
            .arg(migration_scope_arg())
            .arg(query_access_arg())
            .arg(mutation_access_arg())
    }

    fn env_processing(&self, _env: &dyn Environment) -> EnvProcessing {
        EnvProcessing::DoNotProcess
    }

    async fn execute(
        &self,
        matches: &ArgMatches,
        _config: &Config,
        env: Arc<dyn Environment>,
    ) -> Result<()> {
        let path: PathBuf = get_required(matches, "path")?;
        let from_database: bool = matches.get_flag("from-database");
        let database_url = database_value(matches);
        let query_access: bool = query_access_value(matches);
        let mutation_access: bool = mutation_access_value(matches);
        let scope: Option<String> = migration_scope_value(matches);
        let path_str = path.display().to_string();

        if path.exists() {
            return Err(anyhow!(
                "The path '{}' already exists. Please choose a different name.",
                path_str
            ));
        }

        let src_path = path.join("src");
        create_dir_all(&src_path)?;
        let tests_path = path.join("tests");
        create_dir_all(&tests_path)?;

        let mut gitignore_file = File::create(path.join(".gitignore"))?;
        gitignore_file.write_all(GITIGNORE_TEMPLATE)?;

        let mut model_file = File::create(src_path.join("index.exo"))?;
        model_file.write_all(SRC_INDEX_TEMPLATE)?;

        let mut test_file = File::create(tests_path.join("basic-query.exotest"))?;
        test_file.write_all(TESTS_TEST_TEMPLATE)?;

        let mut init_file = File::create(tests_path.join("init.gql"))?;
        init_file.write_all(TESTS_INIT_TEMPLATE)?;

        match which::which("git") {
            Ok(_) => match std::process::Command::new("git").arg("status").output() {
                Ok(output) if output.status.success() => {
                    // Git is already initialized (in a target directory's parent). Following `cargo
                    // new` behavior, we skip the initialization This is useful, for example, if the
                    // user is creating it as a sibling to the frontend repo and the parent of
                    // backend/frontend has git initialized
                }
                _ => {
                    std::process::Command::new("git")
                        .arg("init")
                        .arg(path_str.clone())
                        .output()?;
                }
            },
            Err(_) => {
                // It is not an error to not have git installed, but we should warn the user
                println!("Git is not installed. Skipping repository initialization...");
            }
        }

        if from_database {
            let mut model_file = File::create(src_path.join("index.exo"))?;

            let db_client = open_database(
                database_url.as_deref(),
                TransactionMode::ReadOnly,
                env.as_ref(),
            )
            .await?;
            let db_client = db_client.get_client().await?;

            let table_names = create_exo_model(
                &mut model_file,
                &db_client,
                query_access,
                mutation_access,
                false,
                scope,
            )
            .await?;

            println!("Imported tables:");
            print_imported_tables(table_names, 80);
            println!();
        }

        println!(
            "A new project has been created in the {} directory.",
            path_str.bold().cyan()
        );
        println!(
            "To start the server, run {} {} and then {}!",
            "cd".bold().cyan(),
            path_str.bold().cyan(),
            "exo yolo".bold().green(),
        );

        Ok(())
    }
}
