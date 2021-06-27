//! Subcommands under the `schema` subcommand

use std::path::PathBuf;

use super::Command;

/// Create a database schema from a claytip model
#[derive(Debug)]
pub struct CreateCommand {
    pub model: PathBuf,
}

impl Command for CreateCommand {
    fn run(&self) -> Result<(), String> {
        // let args: Vec<String> = env::args().collect();
        // let model_file = args
        //     .get(1)
        //     .map(|arg| arg.as_str())
        //     .unwrap_or(DEFAULT_MODEL_FILE);
        // let (ast_system, codemap) = parser::parse_file(&model_file);
        // let system = system_builder::build(ast_system, codemap);

        // let schema_stmt = schema::schema_stmt(system.tables);

        // println!("{}", schema_stmt);

        println!("{:#?}", self);
        Ok(())
    }
}
