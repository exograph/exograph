use codemap::{CodeMap, Span};

pub fn null_span() -> Span {
    let mut codemap = CodeMap::new();
    let file = codemap.add_file("".to_string(), "".to_string());
    file.span
}
