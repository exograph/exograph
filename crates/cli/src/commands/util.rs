// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::io::stdin;

use anyhow::anyhow;
use clap::{Arg, ArgMatches, parser::ValueSource};
use colored::Colorize;
use common::env_const::{
    EXO_CORS_DOMAINS, EXO_ENV, EXO_INTROSPECTION, EXO_INTROSPECTION_LIVE_UPDATE,
    EXO_POSTGRES_READ_WRITE,
};
use exo_env::{Environment, MapEnvironment};
use exo_sql::schema::spec::{MigrationScope, MigrationScopeMatches, NameMatching};
use rand::Rng;

pub(super) fn generate_random_string() -> String {
    rand::rng()
        .sample_iter(&rand::distr::Alphanumeric)
        .take(15)
        .map(char::from)
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

pub(crate) fn wait_for_enter(prompt: &str) -> std::io::Result<()> {
    println!("{prompt}");

    let mut line = String::new();
    stdin().read_line(&mut line)?;

    Ok(())
}

pub(crate) fn use_ir_arg() -> Arg {
    Arg::new("use-ir")
        .help("Use the IR file instead of the model file")
        .long("use-ir")
        .required(false)
        .num_args(0)
}

pub(crate) fn compute_migration_scope(scope_value: Option<String>) -> MigrationScope {
    // The value of the form "schema1.table1, schema2.table2, schema3".
    // - wildcards allowed for schema and table names (e.g. "*.table1" or "schema1.*")
    // - table names defaults to '*' (e.g. "schema1" is equivalent to "schema1.*")
    if let Some(scope_value) = scope_value {
        let schema_and_table_names = scope_value
            .trim()
            .split(',')
            .map(|s| {
                let mut parts = s.trim().split('.');
                let schema_name = parts.next().unwrap().trim();
                let table_name = parts.next().unwrap_or("*").trim();
                (
                    NameMatching::new(schema_name),
                    NameMatching::new(table_name),
                )
            })
            .collect::<Vec<_>>();

        MigrationScope::Specified(MigrationScopeMatches(schema_and_table_names))
    } else {
        MigrationScope::FromNewSpec
    }
}

/// Set the environment variables common to the dev and yolo modes.
pub(crate) fn set_dev_yolo_env_vars(env_vars: &mut MapEnvironment, is_yolo: bool) {
    let mode_name = if is_yolo { "yolo" } else { "dev" };

    if env_vars.get(EXO_INTROSPECTION).is_some() {
        println!(
            "{}",
            format!(
                "{} mode ignores EXO_INTROSPECTION. Introspection is always enabled.",
                mode_name
            )
            .yellow()
        );
    }
    env_vars.set(EXO_INTROSPECTION, "true");

    if env_vars.get(EXO_INTROSPECTION_LIVE_UPDATE).is_some() {
        println!(
            "{}",
            format!(
                "{} mode ignores EXO_INTROSPECTION_LIVE_UPDATE. Live update is always enabled.",
                mode_name
            )
            .yellow()
        );
    }
    env_vars.set(EXO_INTROSPECTION_LIVE_UPDATE, "true");

    if env_vars.get(EXO_CORS_DOMAINS).is_some() {
        println!(
            "{}",
            format!(
                "{} mode ignores EXO_CORS_DOMAINS. Using * instead.",
                mode_name
            )
            .yellow()
        );
    }
    env_vars.set(EXO_CORS_DOMAINS, "*");

    if env_vars.get(EXO_ENV).is_some() {
        println!(
            "{}",
            format!(
                "{} mode ignores EXO_ENV. Using {} instead.",
                mode_name, mode_name
            )
            .yellow()
        );
    }
    env_vars.set(EXO_ENV, mode_name);
}

pub(crate) fn read_write_mode(
    matches: &ArgMatches,
    flag_id: &str,
    env_vars: &dyn Environment,
) -> Result<bool, anyhow::Error> {
    let cli_flag = matches.get_flag(flag_id);
    let env_flag = env_vars
        .enabled(EXO_POSTGRES_READ_WRITE, false)
        .map_err(|e| anyhow!("Invalid value for EXO_POSTGRES_READ_WRITE: {}", e))?;

    let cli_arg_source = matches.value_source(flag_id);
    let env_arg_val = env_vars.get(EXO_POSTGRES_READ_WRITE);

    if cli_arg_source == Some(ValueSource::CommandLine)
        && env_arg_val.is_some()
        && cli_flag != env_flag
    {
        anyhow::bail!(
            "Conflicting values for the --{} flag ({}) and the {} env var ({})",
            flag_id,
            cli_flag,
            EXO_POSTGRES_READ_WRITE,
            env_arg_val.unwrap()
        );
    }

    Ok(cli_flag || env_flag)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_migration_scope_from_env() {
        assert_eq!(compute_migration_scope(None), MigrationScope::FromNewSpec);

        assert_eq!(
            compute_migration_scope(Some(
                "schema1.table1,*.table2,schema3.*, schema4".to_string()
            )),
            MigrationScope::Specified(MigrationScopeMatches(vec![
                (NameMatching::new("schema1"), NameMatching::new("table1")),
                (NameMatching::new("*"), NameMatching::new("table2")),
                (NameMatching::new("schema3"), NameMatching::new("*")),
                (NameMatching::new("schema4"), NameMatching::new("*")),
            ]))
        );
    }
}
