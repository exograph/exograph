//! Subcommands under the `schema` subcommand

use std::path::PathBuf;

use payas_parser::{builder::system_builder, parser};

use crate::schema;

use super::Command;

/// Create a database schema from a claytip model
#[derive(Debug)]
pub struct CreateCommand {
    pub model: PathBuf,
}

impl Command for CreateCommand {
    fn run(&self) -> Result<(), String> {
        let (ast_system, codemap) = parser::parse_file(&self.model);
        let system = system_builder::build(ast_system, codemap);

        let schema_stmt = schema::schema_stmt(system.tables);

        println!("{}", schema_stmt);

        Ok(())
    }
}
