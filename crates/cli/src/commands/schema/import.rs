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
use exo_sql::schema::column_spec::{ColumnReferenceSpec, ColumnSpec, ColumnTypeSpec};
use exo_sql::schema::database_spec::DatabaseSpec;
use exo_sql::schema::issue::WithIssues;
use exo_sql::schema::spec::{MigrationScope, MigrationScopeMatches};
use exo_sql::schema::table_spec::TableSpec;
use std::path::PathBuf;

use heck::{ToLowerCamelCase, ToUpperCamelCase};

use exo_sql::schema::issue::Issue;

use crate::commands::command::{database_arg, get, output_arg, CommandDefinition};
use crate::commands::util::migration_scope_from_env;
use crate::config::Config;
use crate::util::open_file_for_output;

use super::util::open_database;

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
    async fn execute(&self, matches: &clap::ArgMatches, _config: &Config) -> Result<()> {
        let output: Option<PathBuf> = get(matches, "output");
        let database_url: Option<String> = get(matches, "database");
        let mut issues = Vec::new();

        let mut writer = open_file_for_output(output.as_deref())?;

        let mut schema = import_schema(database_url, &migration_scope_from_env()).await?;
        let mut model = schema.value.to_model(&mut writer)?;

        issues.append(&mut schema.issues);
        issues.append(&mut model.issues);

        for issue in &issues {
            eprintln!("{issue}");
        }

        if let Some(output) = &output {
            eprintln!("\nExograph model written to `{}`", output.display());
        }

        Ok(())
    }
}

async fn import_schema(
    database_url: Option<String>,
    scope: &MigrationScope,
) -> Result<WithIssues<DatabaseSpec>> {
    let db_client = open_database(database_url.as_deref()).await?;
    let client = db_client.get_client().await?;

    let scope_matches = match scope {
        MigrationScope::Specified(scope) => scope,
        MigrationScope::FromNewSpec => &MigrationScopeMatches::all_schemas(),
    };

    let database = DatabaseSpec::from_live_database(&client, scope_matches).await?;
    Ok(database)
}

pub trait ToModel {
    fn to_model(&self, writer: &mut (dyn std::io::Write + Send)) -> Result<WithIssues<()>>;
}

/// Converts the name of a SQL table to a exograph model name (for example, concert_artist -> ConcertArtist).
fn to_model_name(name: &str) -> String {
    name.to_upper_camel_case()
}

impl ToModel for DatabaseSpec {
    /// Converts the schema specification to a exograph file.
    fn to_model(&self, writer: &mut (dyn std::io::Write + Send)) -> Result<WithIssues<()>> {
        let mut issues = Vec::new();

        writeln!(writer, "@postgres")?;
        writeln!(writer, "module Database {{")?;

        for table in &self.tables {
            let mut model = table.to_model(writer)?;
            issues.append(&mut model.issues);
            writeln!(writer)?;
        }

        writeln!(writer, "}}")?;

        Ok(WithIssues { value: (), issues })
    }
}

impl ToModel for TableSpec {
    fn to_model(&self, writer: &mut (dyn std::io::Write + Send)) -> Result<WithIssues<()>> {
        let mut issues = Vec::new();

        match &self.name.schema {
            Some(schema) => writeln!(
                writer,
                "\t@table(name=\"{}\", schema=\"{}\")",
                self.name.name, schema
            )?,
            None => writeln!(writer, "\t@table(\"{}\")", self.name.name)?,
        };

        writeln!(writer, "\ttype {} {{", to_model_name(&self.name.name))?;

        for column in &self.columns {
            let mut model = column.to_model(writer)?;
            issues.append(&mut model.issues);
        }

        writeln!(writer, "\t}}")?;

        // not a robust check
        if self.name.name.ends_with('s') {
            issues.push(Issue::Hint(format!(
                "model name `{}` should be changed to singular",
                to_model_name(&self.name.name)
            )));
        }

        Ok(WithIssues { value: (), issues })
    }
}

impl ToModel for ColumnSpec {
    /// Converts the column specification to a exograph model.
    fn to_model(&self, writer: &mut (dyn std::io::Write + Send)) -> Result<WithIssues<()>> {
        let mut issues = Vec::new();

        // [@pk] [type-annotations] [name]: [data-type] = [default-value]

        let pk_str = if self.is_pk { "@pk " } else { "" };
        write!(writer, "\t\t{}", pk_str)?;
        let (mut data_type, annots) = self.typ.to_model();

        write!(writer, "{}", &annots)?;

        write!(writer, "{}: ", self.name.to_lower_camel_case())?;

        if let ColumnTypeSpec::ColumnReference(ColumnReferenceSpec {
            foreign_table_name, ..
        }) = &self.typ
        {
            data_type = to_model_name(&data_type);

            issues.push(Issue::Hint(format!(
                "consider adding a field to `{}` of type `[{}]` to create a one-to-many relationship",
                foreign_table_name.fully_qualified_name(), to_model_name(&self.name),
            )));
        }

        if self.is_nullable {
            data_type += "?"
        }

        let autoinc_str = if self.is_auto_increment {
            " = autoIncrement()"
        } else {
            ""
        };

        writeln!(writer, "{}{}{}", data_type, &annots, autoinc_str)?;

        Ok(WithIssues { value: (), issues })
    }
}
