// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::path::Path;

#[cfg(not(target_family = "wasm"))]
use std::path::PathBuf;

use codemap::{CodeMap, Span};
use codemap_diagnostic::{Diagnostic, Level, SpanLabel, SpanStyle};
use tree_sitter_c2rust::Node;

use crate::{
    ast::ast_types::{AstSystem, Untyped},
    error::ParserError,
};

#[cfg(not(target_family = "wasm"))]
use crate::FileSystem;

mod converter;
mod sitter_ffi;

use self::converter::{convert_root, parse};

fn span_from_node(source_span: Span, node: Node<'_>) -> Span {
    source_span.subspan(node.start_byte() as u64, node.end_byte() as u64)
}

#[cfg(not(target_family = "wasm"))]
/// Parse a file and return the AST
///
/// # Arguments
/// * `input_file` - The file to parse
/// * `codemap` - The codemap to accumulate errors
pub fn parse_file(
    input_file: impl AsRef<Path>,
    file_system: &impl FileSystem,
    codemap: &mut CodeMap,
) -> Result<AstSystem<Untyped>, ParserError> {
    let mut already_parsed = vec![];
    _parse_file(input_file, file_system, codemap, &mut already_parsed)
}

#[cfg(not(target_family = "wasm"))]
/// Parse a file and return the AST.
///
/// Takes care of dealing with potentially recursive imports.
///
/// # Arguments
/// * `input_file` - The file to parse
/// * `codemap` - The codemap to accumulate errors
/// * already_parsed - a vector of files that have already been parsed. Used to ensure that recursive imports do not cause an infinite loop
fn _parse_file(
    input_file: impl AsRef<Path>,
    file_system: &impl FileSystem,
    codemap: &mut CodeMap,
    already_parsed: &mut Vec<PathBuf>,
) -> Result<AstSystem<Untyped>, ParserError> {
    let input_file_path = Path::new(input_file.as_ref());

    if !file_system.exists(input_file.as_ref()) {
        return Err(ParserError::FileNotFound(
            input_file.as_ref().display().to_string(),
        ));
    }

    let source = file_system.read_file(input_file.as_ref())?;
    let mut system = parse_str(&source, codemap, input_file_path)?;

    // add to already parsed list since we're parsing it currently
    already_parsed.push(input_file_path.to_path_buf());

    for import in system.imports.iter() {
        if !already_parsed.contains(import) {
            // parse import
            let mut imported_system = _parse_file(import, file_system, codemap, already_parsed)?;

            // merge import into system
            system.types.append(&mut imported_system.types);
            system.modules.append(&mut imported_system.modules);
        }
    }

    Ok(system)
}

pub fn parse_str(
    source: &str,
    codemap: &mut CodeMap,
    input_file_name: impl AsRef<Path>,
) -> Result<AstSystem<Untyped>, ParserError> {
    let source_span = codemap
        .add_file(
            input_file_name.as_ref().display().to_string(),
            source.to_string(),
        )
        .span;
    let parsed = parse(source).unwrap();

    let root_node = parsed.root_node();

    if root_node.has_error() {
        let mut errors = vec![];
        collect_parsing_errors(root_node, source_span, &mut errors);
        return Err(ParserError::Diagnosis(errors));
    };

    convert_root(
        parsed.root_node(),
        source.as_bytes(),
        source_span,
        input_file_name.as_ref(),
    )
}

fn collect_parsing_errors(node: Node, source_span: Span, errors: &mut Vec<Diagnostic>) {
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
                message: format!("Unexpected token: \"{tok}\""),
                code: Some("S000".to_string()),
                spans: vec![SpanLabel {
                    span: span_from_node(source_span, expl).subspan(1, 1),
                    style: SpanStyle::Primary,
                    label: Some(format!("unexpected \"{tok}\"")),
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
            .for_each(|c| collect_parsing_errors(c, source_span, errors));
    }
}
