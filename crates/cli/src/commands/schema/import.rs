// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use anyhow::Result;
use async_trait::async_trait;
use clap::Command;
use exo_sql::schema::issue::WithIssues;
use exo_sql::schema::spec::from_db;
use exo_sql::{Database, DatabaseClient};
use std::{io::Write, path::PathBuf};

use heck::ToUpperCamelCase;

use exo_sql::schema::issue::Issue;
use exo_sql::{PhysicalColumn, PhysicalColumnType, PhysicalTable};

use crate::commands::command::{database_arg, get, output_arg, CommandDefinition};
use crate::util::open_file_for_output;

pub(super) struct ImportCommandDefinition {}

#[async_trait]
impl CommandDefinition for ImportCommandDefinition {
    fn command(&self) -> clap::Command {
        Command::new("import")
            .about("Create exograph model file based on a database schema")
            .arg(database_arg())
            .arg(output_arg())
    }

    /// Create a exograph model file based on a database schema
    async fn execute(&self, matches: &clap::ArgMatches) -> Result<()> {
        let output: Option<PathBuf> = get(matches, "output");
        let mut issues = Vec::new();
        let mut schema = import_schema().await?;
        let mut model = schema.value.to_model(&schema.value);

        issues.append(&mut schema.issues);
        issues.append(&mut model.issues);

        let mut buffer: Box<dyn Write> = open_file_for_output(output.as_deref())?;
        buffer.write_all(schema.value.to_model(&schema.value).value.as_bytes())?;

        for issue in &issues {
            eprintln!("{issue}");
        }

        if let Some(output) = &output {
            eprintln!("\nExograph model written to `{}`", output.display());
        }

        Ok(())
    }
}

async fn import_schema() -> Result<WithIssues<Database>> {
    let database_client = DatabaseClient::from_env(Some(1))?; // TODO: error handling here
    let client = database_client.get_client().await?;
    let database = from_db(&client).await?;
    Ok(database)
}

pub trait ToModel {
    fn to_model(&self, database: &Database) -> WithIssues<String>;
}

/// Converts the name of a SQL table to a exograph model name (for example, concert_artist -> ConcertArtist).
fn to_model_name(name: &str) -> String {
    name.to_upper_camel_case()
}

impl ToModel for Database {
    /// Converts the schema specification to a exograph file.
    fn to_model(&self, database: &Database) -> WithIssues<String> {
        let mut issues = Vec::new();
        let stmt = self
            .tables()
            .iter()
            .map(|(_, table)| {
                let mut model = table.to_model(database);
                issues.append(&mut model.issues);
                format!("{}\n\n", model.value)
            })
            .collect();

        WithIssues {
            value: stmt,
            issues,
        }
    }
}

impl ToModel for PhysicalTable {
    /// Converts the table specification to a exograph model.
    fn to_model(&self, database: &Database) -> WithIssues<String> {
        let mut issues = Vec::new();

        let table_annot = format!("@table(\"{}\")", self.name);
        let column_stmts = self
            .columns
            .iter()
            .map(|c| {
                let mut model = c.to_model(database);
                issues.append(&mut model.issues);
                format!("  {}\n", model.value)
            })
            .collect::<String>();

        // not a robust check
        if self.name.ends_with('s') {
            issues.push(Issue::Hint(format!(
                "model name `{}` should be changed to singular",
                to_model_name(&self.name)
            )));
        }

        WithIssues {
            value: format!(
                "{}\nmodel {} {{\n{}}}",
                table_annot,
                to_model_name(&self.name),
                column_stmts
            ),
            issues,
        }
    }
}

impl ToModel for PhysicalColumn {
    /// Converts the column specification to a exograph model.
    fn to_model(&self, database: &Database) -> WithIssues<String> {
        let mut issues = Vec::new();

        let pk_str = if self.is_pk { " @pk" } else { "" };
        let autoinc_str = if self.is_auto_increment {
            " = autoIncrement()"
        } else {
            ""
        };

        let (mut data_type, annots) = self.typ.to_model();
        if let PhysicalColumnType::ColumnReference { ref_table_name, .. } = &self.typ {
            data_type = to_model_name(&data_type);

            issues.push(Issue::Hint(format!(
                "consider adding a field to `{}` of type `[{}]` to create a one-to-many relationship",
                ref_table_name, to_model_name(&database.get_table(self.table_id).name),
            )));
        }

        if self.is_nullable {
            data_type += "?"
        }

        WithIssues {
            value: format!(
                "{}: {}{}{}",
                self.name,
                data_type + &annots,
                autoinc_str,
                pk_str,
            ),
            issues: Vec::new(),
        }
    }
}
