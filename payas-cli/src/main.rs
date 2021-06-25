use std::env;

use payas_parser::{builder::system_builder, parser};

mod schema;

const DEFAULT_MODEL_FILE: &str = "index.clay";

fn main() {
    let args: Vec<String> = env::args().collect();
    let model_file = args
        .get(1)
        .map(|arg| arg.as_str())
        .unwrap_or(DEFAULT_MODEL_FILE);
    let (ast_system, codemap) = parser::parse_file(&model_file);
    let system = system_builder::build(ast_system, codemap);

    let schema_stmt = schema::schema_stmt(system.tables);

    println!("{}", schema_stmt);
}
