use std::io::Write;
use std::path::PathBuf;
use tree_sitter_cli::generate;

fn main() {
    let dir = tempfile::Builder::new()
        .prefix("grammar")
        .tempdir()
        .unwrap();
    let grammar_file = dir.path().join("parser.c");
    let mut f = std::fs::File::create(grammar_file).unwrap();

    let grammar = generate::load_grammar_file(&PathBuf::from("./grammar/grammar.js")).unwrap();
    let (grammar_name, grammar_c) = generate::generate_parser_for_grammar(&grammar).unwrap();
    f.write_all(grammar_c.as_bytes()).unwrap();
    drop(f);

    let header_dir = dir.path().join("tree_sitter");
    std::fs::create_dir(&header_dir).unwrap();
    let mut parser_file = std::fs::File::create(header_dir.join("parser.h")).unwrap();
    parser_file
        .write_all(tree_sitter::PARSER_HEADER.as_bytes())
        .unwrap();
    drop(parser_file);

    cc::Build::new()
        .include(&dir)
        .file(dir.path().join("parser.c"))
        .compile(&grammar_name);
}
