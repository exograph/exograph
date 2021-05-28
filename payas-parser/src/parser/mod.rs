use std::{fs, path::Path};

use crate::ast::ast_types::*;

mod converter;
mod sitter_ffi;

use self::converter::*;

pub fn parse_file<'a, P: AsRef<Path>>(path: P) -> AstSystem {
    let file_content = fs::read_to_string(path).unwrap();
    let parsed = parse(file_content.as_str()).unwrap();
    convert_root(parsed.root_node(), file_content.as_bytes())
}

pub fn parse_str(str: &str) -> AstSystem {
    let parsed = parse(str).unwrap();
    convert_root(parsed.root_node(), str.as_bytes())
}
