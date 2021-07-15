//! Subcommands under the `schema` subcommand

use anyhow::Result;
use std::path::PathBuf;

use payas_parser::{builder, parser};

use crate::schema;

use super::Command;

/// Create a database schema from a claytip model
pub struct CreateCommand {
    pub model: PathBuf,
}

impl Command for CreateCommand {
    fn run(&self) -> Result<()> {
        let (ast_system, codemap) = parser::parse_file(&self.model);
        let system = builder::build(ast_system, codemap)?;

        let schema_stmt = schema::schema_stmt(system.tables);

        println!("{}", schema_stmt);

        Ok(())
    }
}

/// Verify that a schema is compatible with a claytip model
pub struct VerifyCommand {
    pub model: PathBuf,
    pub database: String,
}

impl Command for VerifyCommand {
    fn run(&self) -> Result<()> {
        todo!("Implmement verify command");
    }
}
