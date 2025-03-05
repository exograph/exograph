// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::io::stdin;

use clap::Arg;
use exo_sql::schema::spec::{MigrationScope, MigrationScopeMatches, NameMatching};
use rand::Rng;

pub(super) fn generate_random_string() -> String {
    rand::thread_rng()
        .sample_iter(&rand::distributions::Alphanumeric)
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
