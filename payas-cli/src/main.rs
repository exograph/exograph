use std::env;

use payas_model::sql::{column::PhysicalColumn, PhysicalTable};
use payas_parser::{builder::system_builder, parser};

fn main() {
    println!("Payas Client");

    let args: Vec<String> = env::args().collect();
    let ast_system = parser::parse_file(&args[1]);
    let system = system_builder::build(ast_system.unwrap());

    let tables = system.tables;

    let table_stmts = tables
        .iter()
        .map(|table| create_table(table.1))
        .collect::<Vec<_>>()
        .join("\n\n");

    println!("{}", table_stmts);
}

fn create_table(table: &PhysicalTable) -> String {
    let column_stmts = table
        .columns
        .iter()
        .map(create_column)
        .collect::<Vec<_>>()
        .join(",\n\t");

    format!("CREATE TABLE {} (\n\t{}\n);", table.name, column_stmts)
}

fn create_column(column: &PhysicalColumn) -> String {
    let pk_str = if column.is_pk { " PRIMARY KEY" } else { "" };

    let references_str = match column.references {
        Some(ref references) => format!(" REFERENCES {}", references.table_name),
        None => "".to_string(),
    };

    format!(
        "{} {}{}{}",
        column.column_name,
        column.typ.db_type(column.is_autoincrement),
        pk_str,
        references_str
    )
}
