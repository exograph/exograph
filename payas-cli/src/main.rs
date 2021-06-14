use std::env;

use payas_parser::{builder::system_builder, parser};

mod schema;

fn main() {
    let args: Vec<String> = env::args().collect();
    let (ast_system, codemap) = parser::parse_file(&args[1]);
    let system = system_builder::build(ast_system, codemap);

    let schema_stmt = schema::schema_stmt(system.tables);

    println!("{}", schema_stmt);
}
