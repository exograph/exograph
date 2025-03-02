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
use exo_sql::{FloatBits, IntBits, PhysicalTableName};
use std::collections::HashMap;
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

        let mut context = ImportContext::new();
        let mut schema = import_schema(database_url, &migration_scope_from_env()).await?;
        let mut model = schema.value.to_model(&mut context, &mut writer)?;

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

struct ImportContext {
    table_name_to_model_name: HashMap<PhysicalTableName, String>,
}

impl ImportContext {
    fn new() -> Self {
        Self {
            table_name_to_model_name: HashMap::new(),
        }
    }

    fn model_name(&self, table_name: &PhysicalTableName) -> &str {
        self.table_name_to_model_name.get(table_name).unwrap()
    }

    fn has_standard_mapping(&self, table_name: &PhysicalTableName) -> bool {
        self.model_name(table_name) != table_name.name.to_upper_camel_case()
    }

    /// Converts the name of a SQL table to a exograph model name (for example, concert_artist -> ConcertArtist).
    fn add_table(&mut self, table_name: &PhysicalTableName) {
        let singular_name = pluralizer::pluralize(&table_name.name, 1, false);

        // If the singular name is the same (for example, uncountable nouns such as 'news'), use the original name.
        let model_name = if singular_name == table_name.name {
            table_name.name.to_upper_camel_case()
        } else {
            singular_name.to_upper_camel_case()
        };

        self.table_name_to_model_name
            .insert(table_name.clone(), model_name.clone());
    }
}

trait ToModel {
    fn to_model(
        &self,
        context: &mut ImportContext,
        writer: &mut (dyn std::io::Write + Send),
    ) -> Result<WithIssues<()>>;
}

impl ToModel for DatabaseSpec {
    /// Converts the schema specification to a exograph file.
    fn to_model(
        &self,
        context: &mut ImportContext,
        writer: &mut (dyn std::io::Write + Send),
    ) -> Result<WithIssues<()>> {
        let mut issues = Vec::new();

        writeln!(writer, "@postgres")?;
        writeln!(writer, "module Database {{")?;

        for table in &self.tables {
            context.add_table(&table.name);
        }

        for table in &self.tables {
            let mut model = table.to_model(context, writer)?;
            issues.append(&mut model.issues);
            writeln!(writer)?;
        }

        writeln!(writer, "}}")?;

        Ok(WithIssues { value: (), issues })
    }
}

impl ToModel for TableSpec {
    fn to_model(
        &self,
        context: &mut ImportContext,
        writer: &mut (dyn std::io::Write + Send),
    ) -> Result<WithIssues<()>> {
        let mut issues = Vec::new();

        if !context.has_standard_mapping(&self.name) {
            match &self.name.schema {
                Some(schema) => writeln!(
                    writer,
                    "\t@table(name=\"{}\", schema=\"{}\")",
                    self.name.name, schema
                )?,
                None => writeln!(writer, "\t@table(\"{}\")", self.name.name)?,
            };
        }

        writeln!(writer, "\ttype {} {{", context.model_name(&self.name))?;

        for column in &self.columns {
            let mut model = column.to_model(context, writer)?;
            issues.append(&mut model.issues);
        }

        writeln!(writer, "\t}}")?;

        Ok(WithIssues { value: (), issues })
    }
}

impl ToModel for ColumnSpec {
    /// Converts the column specification to a exograph model.
    fn to_model(
        &self,
        context: &mut ImportContext,
        writer: &mut (dyn std::io::Write + Send),
    ) -> Result<WithIssues<()>> {
        let mut issues = Vec::new();

        // [@pk] [type-annotations] [name]: [data-type] = [default-value]

        let pk_str = if self.is_pk { "@pk " } else { "" };
        write!(writer, "\t\t{}", pk_str)?;
        let (mut data_type, annots) = to_model(&self.typ, context);

        write!(writer, "{}", &annots)?;

        write!(writer, "{}: ", self.name.to_lower_camel_case())?;

        if let ColumnTypeSpec::ColumnReference(ColumnReferenceSpec {
            foreign_table_name, ..
        }) = &self.typ
        {
            // data_type = context.model_name(foreign_table_name);

            issues.push(Issue::Hint(format!(
                "consider adding a field to `{}` of type `[{}]` to create a one-to-many relationship",
                foreign_table_name.fully_qualified_name(), data_type,
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

fn to_model(column_type: &ColumnTypeSpec, context: &mut ImportContext) -> (String, String) {
    match column_type {
        ColumnTypeSpec::Int { bits } => (
            "Int".to_string(),
            match bits {
                IntBits::_16 => " @bits16",
                IntBits::_32 => "",
                IntBits::_64 => " @bits64",
            }
            .to_string(),
        ),

        ColumnTypeSpec::Float { bits } => (
            "Float".to_string(),
            match bits {
                FloatBits::_24 => " @singlePrecision",
                FloatBits::_53 => " @doublePrecision",
            }
            .to_owned(),
        ),

        ColumnTypeSpec::Numeric { precision, scale } => ("Numeric".to_string(), {
            let precision_part = precision
                .map(|p| format!("@precision({p})"))
                .unwrap_or_default();

            let scale_part = scale.map(|s| format!("@scale({s})")).unwrap_or_default();

            format!(" {precision_part} {scale_part}")
        }),

        ColumnTypeSpec::String { max_length } => (
            "String".to_string(),
            match max_length {
                Some(max_length) => format!(" @maxLength({max_length})"),
                None => "".to_string(),
            },
        ),

        ColumnTypeSpec::Boolean => ("Boolean".to_string(), "".to_string()),

        ColumnTypeSpec::Timestamp {
            timezone,
            precision,
        } => (
            if *timezone {
                "Instant"
            } else {
                "LocalDateTime"
            }
            .to_string(),
            match precision {
                Some(precision) => format!(" @precision({precision})"),
                None => "".to_string(),
            },
        ),

        ColumnTypeSpec::Time { precision } => (
            "LocalTime".to_string(),
            match precision {
                Some(precision) => format!(" @precision({precision})"),
                None => "".to_string(),
            },
        ),

        ColumnTypeSpec::Date => ("LocalDate".to_string(), "".to_string()),

        ColumnTypeSpec::Json => ("Json".to_string(), "".to_string()),
        ColumnTypeSpec::Blob => ("Blob".to_string(), "".to_string()),
        ColumnTypeSpec::Uuid => ("Uuid".to_string(), "".to_string()),
        ColumnTypeSpec::Vector { size } => ("Vector".to_string(), format!("@size({size})",)),

        ColumnTypeSpec::Array { typ } => {
            let (data_type, annotations) = to_model(typ, context);
            (format!("[{data_type}]"), annotations)
        }

        ColumnTypeSpec::ColumnReference(ColumnReferenceSpec {
            foreign_table_name, ..
        }) => (
            context.model_name(foreign_table_name).to_string(),
            "".to_string(),
        ),
    }
}
