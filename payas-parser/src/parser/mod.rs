use std::{fs, path::Path};

use codemap::CodeMap;

use crate::ast::ast_types::*;

mod converter;
mod sitter_ffi;

use self::converter::*;

pub fn parse_file<P: AsRef<Path>>(path: P) -> (AstSystem, CodeMap) {
    let file_content = fs::read_to_string(path.as_ref().clone()).unwrap();
    let mut codemap = CodeMap::new();
    let file_span = codemap.add_file(path.as_ref().to_str().unwrap().to_string(), file_content.clone()).span;
    let parsed = parse(file_content.as_str()).unwrap();
    (convert_root(parsed.root_node(), file_content.as_bytes(), file_span), codemap)
}

pub fn parse_str(str: &str) -> (AstSystem, CodeMap) {
    let mut codemap = CodeMap::new();
    let file_span = codemap.add_file("input.payas".to_string(), str.to_string()).span;
    let parsed = parse(str).unwrap();
    (convert_root(parsed.root_node(), str.as_bytes(), file_span), codemap)
}
