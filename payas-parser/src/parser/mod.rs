use std::{fs, path::Path};

use codemap::{CodeMap, Span};
use codemap_diagnostic::{Diagnostic, Level, SpanLabel, SpanStyle};
use tree_sitter::Node;

use crate::{ast::ast_types::*, error::ParserError};

mod converter;
mod sitter_ffi;

use self::converter::*;

fn span_from_node(source_span: Span, node: Node<'_>) -> Span {
    source_span.subspan(node.start_byte() as u64, node.end_byte() as u64)
}

pub fn parse_file<P: AsRef<Path>>(input_file: P) -> Result<AstSystem<Untyped>, ParserError> {
    if !Path::new(input_file.as_ref()).exists() {
        return Err(ParserError::FileNotFound(
            input_file.as_ref().display().to_string(),
        ));
    }
    let source = fs::read_to_string(input_file.as_ref())?;
    parse_str(&source, input_file)
}

pub fn parse_str<P: AsRef<Path>>(
    source: &str,
    input_file_name: P,
) -> Result<AstSystem<Untyped>, ParserError> {
    let mut codemap = CodeMap::new();
    let source_span = codemap
        .add_file(
            input_file_name.as_ref().to_str().unwrap().to_string(),
            source.to_string(),
        )
        .span;
    let parsed = parse(source).unwrap();

    let root_node = parsed.root_node();

    if root_node.has_error() {
        let mut errors = vec![];
        collect_parsing_errors(root_node, source.as_bytes(), source_span, &mut errors);
        return Err(ParserError::Diagosis(errors));
    };

    convert_root(
        parsed.root_node(),
        source.as_bytes(),
        source_span,
        input_file_name.as_ref(),
    )
}

fn collect_parsing_errors(
    node: Node,
    source: &[u8],
    source_span: Span,
    errors: &mut Vec<Diagnostic>,
) {
    if node.is_error() {
        let expl = node.child(0).unwrap();
        let sexp = node.to_sexp();
        if sexp.starts_with("(ERROR (UNEXPECTED") {
            let mut tok_getter = sexp.chars();
            for _ in 0.."(ERROR (UNEXPECTED '".len() {
                tok_getter.next();
            }
            for _ in 0.."'))".len() {
                tok_getter.next_back();
            }
            let tok = tok_getter.as_str();

            errors.push(Diagnostic {
                level: Level::Error,
                message: format!("Unexpected token: \"{}\"", tok),
                code: Some("S000".to_string()),
                spans: vec![SpanLabel {
                    span: span_from_node(source_span, expl).subspan(1, 1),
                    style: SpanStyle::Primary,
                    label: Some(format!("unexpected \"{}\"", tok)),
                }],
            })
        } else {
            errors.push(Diagnostic {
                level: Level::Error,
                message: format!("Unexpected token: \"{}\"", expl.kind()),
                code: Some("S000".to_string()),
                spans: vec![SpanLabel {
                    span: span_from_node(source_span, expl),
                    style: SpanStyle::Primary,
                    label: Some(format!("unexpected \"{}\"", expl.kind())),
                }],
            })
        }
    } else if node.is_missing() {
        errors.push(Diagnostic {
            level: Level::Error,
            message: format!("Missing token: \"{}\"", node.kind()),
            code: Some("S000".to_string()),
            spans: vec![SpanLabel {
                span: span_from_node(source_span, node),
                style: SpanStyle::Primary,
                label: Some(format!("missing \"{}\"", node.kind())),
            }],
        })
    } else {
        let mut cursor = node.walk();
        node.children(&mut cursor)
            .for_each(|c| collect_parsing_errors(c, source, source_span, errors));
    }
}
