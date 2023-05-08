// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::fs::{create_dir_all, File};
use std::io::Write;
use std::path::PathBuf;

use anyhow::{anyhow, Result};
use clap::{ArgMatches, Command};

use super::command::{get_required, new_project_arg, CommandDefinition};

static SRC_INDEX_TEMPLATE: &[u8] = include_bytes!("templates/exo-new/src/index.exo");
static TESTS_TEST_TEMPLATE: &[u8] = include_bytes!("templates/exo-new/tests/basic-query.exotest");
static TESTS_INIT_TEMPLATE: &[u8] = include_bytes!("templates/exo-new/tests/init.gql");
static GITIGNORE_TEMPLATE: &[u8] = include_bytes!("templates/exo-new/gitignore");

pub struct NewCommandDefinition {}

impl CommandDefinition for NewCommandDefinition {
    fn command(&self) -> Command {
        Command::new("new")
            .about("Create a new Exograph project")
            .arg(new_project_arg())
    }

    fn execute(&self, matches: &ArgMatches) -> Result<()> {
        let path: PathBuf = get_required(matches, "path")?;

        if path.exists() {
            return Err(anyhow!(
                "The path '{}' already exists. Please choose a different name.",
                path.display()
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

        Ok(())
    }
}
