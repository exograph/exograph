//! Subcommands under the `model` subcommand

use anyhow::Result;
use payas_model::spec::ToModel;
use payas_sql::schema::issue::WithIssues;
use payas_sql::{schema::spec::SchemaSpec, Database};
use std::{fs::File, io::Write, path::PathBuf, time::SystemTime};

use super::command::Command;

/// Create a claytip model file based on a database schema
pub struct ImportCommand {
    pub output: PathBuf,
}

impl Command for ImportCommand {
    fn run(&self, _system_start_time: Option<SystemTime>) -> Result<()> {
        // Create runtime and make the rest of this an async block
        // (then block on it)
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_io()
            .build()
            .unwrap();

        let mut issues = Vec::new();
        let mut schema = rt.block_on(import_schema())?;
        let mut model = schema.value.to_model();

        issues.append(&mut schema.issues);
        issues.append(&mut model.issues);

        if self.output.exists() {
            Err(anyhow::anyhow!("File {} already exists. Rerun after removing that file or specify a different output file using the -o option", self.output.display()))
        } else {
            File::create(&self.output)?.write_all(schema.value.to_model().value.as_bytes())?;
            for issue in &issues {
                println!("{}", issue);
            }

            println!("\nClaytip model written to `{}`", self.output.display());
            Ok(())
        }
    }
}

async fn import_schema() -> Result<WithIssues<SchemaSpec>> {
    let database = Database::from_env(Some(1))?; // TODO: error handling here
    let client = database.get_client().await?;
    let schema = SchemaSpec::from_db(&client).await?;
    Ok(schema)
}
