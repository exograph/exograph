// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::convert::TryInto;
#[cfg(not(target_family = "wasm"))]
use std::ffi::OsStr;
use std::path::PathBuf;
use std::{collections::HashMap, path::Path};

use codemap::Span;
#[cfg(not(target_family = "wasm"))]
use codemap_diagnostic::{Diagnostic, Level, SpanLabel, SpanStyle};
use core_model_builder::ast::ast_types::{
    AstEnum, AstEnumField, AstFragmentReference, FieldSelectionElement, Identifier,
};
use tree_sitter_c2rust::{Node, Tree, TreeCursor};

use super::{sitter_ffi, span_from_node};
use crate::ast::ast_types::{
    AstAnnotation, AstAnnotationParams, AstArgument, AstExpr, AstField, AstFieldDefault,
    AstFieldDefaultKind, AstFieldType, AstInterceptor, AstMethod, AstModel, AstModelKind,
    AstModule, AstSystem, FieldSelection, LogicalOp, RelationalOp, Untyped,
};
use crate::error::ParserError;

pub fn parse(input: &str) -> Option<Tree> {
    let mut parser = tree_sitter_c2rust::Parser::new();
    parser.set_language(&sitter_ffi::language()).unwrap();
    parser.parse(input, None)
}

pub fn convert_root(
    node: Node,
    source: &[u8],
    source_span: Span,
    filepath: &Path,
) -> Result<AstSystem<Untyped>, ParserError> {
    assert_eq!(node.kind(), "source_file");

    fn collect_declaration_doc_comments(
        node: Node,
        source: &[u8],
        source_span: Span,
    ) -> Option<String> {
        // Take all the declaration doc comments and join them with a newline. This allows developers to, for example, write a declaration doc comment
        // in each imported file.
        let mut cursor = node.walk();
        let doc_comments = node
            .children(&mut cursor)
            .filter(|n| n.kind() == "declaration")
            .filter_map(|c| convert_declaration_doc_comments(c, source, source_span))
            .collect::<Vec<_>>();

        if doc_comments.is_empty() {
            None
        } else {
            Some(doc_comments.join("\n"))
        }
    }

    let mut cursor = node.walk();
    Ok(AstSystem {
        declaration_doc_comments: collect_declaration_doc_comments(node, source, source_span),
        types: node
            .children(&mut cursor)
            .filter(|n| n.kind() == "declaration")
            .filter_map(|c| convert_declaration_to_context(c, source, source_span))
            .collect::<Vec<_>>(),
        modules: node
            .children(&mut cursor)
            .filter(|n| n.kind() == "declaration")
            .filter_map(|c| convert_declaration_to_module(c, source, source_span, filepath))
            .collect::<Vec<_>>(),
        imports: {
            let imports = node
                .children(&mut cursor)
                .filter(|n| n.kind() == "declaration")
                .map(|c| -> Result<Option<PathBuf>, ParserError> {
                    let first_child = c.child(0).unwrap();

                    if first_child.kind() == "import" {
                        let path_str = text_child(
                            first_child.child_by_field_name("path").unwrap(),
                            source,
                            "value",
                        );

                        // Create a path relative to the current file
                        let mut import_path = filepath.to_owned();
                        import_path.pop();
                        import_path.push(path_str);

                        #[cfg(not(target_family = "wasm"))]
                        {
                            fn compute_diagnosis(
                                import_path: PathBuf,
                                source_span: Span,
                                node: Node,
                            ) -> ParserError {
                                ParserError::Diagnosis(vec![Diagnostic {
                                    level: Level::Error,
                                    message: format!(
                                        "File not found {}",
                                        import_path.to_string_lossy()
                                    ),
                                    code: Some("C000".to_string()),
                                    spans: vec![SpanLabel {
                                        span: span_from_node(source_span, node),
                                        style: SpanStyle::Primary,
                                        label: None,
                                    }],
                                }])
                            }

                            let check_existence =
                                |import_path: PathBuf| -> Result<Option<PathBuf>, ParserError> {
                                    match import_path.canonicalize() {
                                        Ok(path) if path.is_file() => Ok(Some(path)),
                                        _ => Err(compute_diagnosis(
                                            import_path,
                                            source_span,
                                            first_child,
                                        )),
                                    }
                                };

                            // Resolve the import path
                            // 1. If the path exists and it is a file, return the path
                            // 2. If the path exists and it is a directory, check for <path>/index.exo
                            // 3. If the path doesn't exist, check for <path>.exo
                            match import_path.canonicalize() {
                                Ok(path) if path.is_file() => Ok(Some(path)),
                                Ok(path) if path.is_dir() => {
                                    // If the path is a directory, try to find <directory>/index.exo
                                    let with_index_exo = path.join("index.exo");
                                    check_existence(with_index_exo)
                                }
                                _ => {
                                    // If no extension is given, try if a file with the same name but with ".exo" extension exists.
                                    if import_path.extension() == Some(OsStr::new("exo")) {
                                        // Already has the .exo extension, so further checks are not necessary (it is a failure since the file does not exist).
                                        Err(compute_diagnosis(
                                            import_path,
                                            source_span,
                                            first_child,
                                        ))
                                    } else {
                                        let with_extension = import_path.with_extension("exo");
                                        check_existence(with_extension)
                                    }
                                }
                            }
                        }

                        #[cfg(target_family = "wasm")]
                        {
                            panic!("Imports are not supported on WebAssembly")
                        }
                    } else {
                        Ok(None)
                    }
                })
                .collect::<Result<Vec<_>, _>>();
            imports?.into_iter().flatten().collect()
        },
    })
}

fn convert_declaration_to_context(
    node: Node,
    source: &[u8],
    source_span: Span,
) -> Option<AstModel<Untyped>> {
    assert_eq!(node.kind(), "declaration");
    let first_child = node.child(0).unwrap();

    if first_child.kind() == "context" {
        Some(convert_model(
            first_child,
            source,
            source_span,
            AstModelKind::Context,
        ))
    } else {
        None
    }
}

fn convert_declaration_to_module(
    node: Node,
    source: &[u8],
    source_span: Span,
    filepath: &Path,
) -> Option<AstModule<Untyped>> {
    assert_eq!(node.kind(), "declaration");
    let first_child = node.child(0).unwrap();

    if first_child.kind() == "module" {
        let module = convert_module(first_child, source, source_span, filepath);
        Some(module)
    } else {
        None
    }
}

fn convert_model(
    node: Node,
    source: &[u8],
    source_span: Span,
    kind: AstModelKind,
) -> AstModel<Untyped> {
    assert!(node.kind() == "type" || node.kind() == "context" || node.kind() == "fragment");

    let mut cursor = node.walk();

    let (fields, fragment_references) = convert_fields_and_fragments(
        node.child_by_field_name("body").unwrap(),
        source,
        source_span,
    );

    AstModel {
        name: text_child(node, source, "name"),
        kind,
        fields,
        fragment_references,
        annotations: node
            .children_by_field_name("annotation", &mut cursor)
            .map(|c| convert_annotation(c, source, source_span))
            .collect(),
        doc_comments: convert_doc_comments(node, source, source_span),
        span: span_from_node(source_span, node.child_by_field_name("name").unwrap()),
    }
}

fn convert_enum(node: Node, source: &[u8], source_span: Span) -> AstEnum<Untyped> {
    assert_eq!(node.kind(), "enum");

    let mut cursor = node.walk();

    let fields = node
        .child_by_field_name("body")
        .unwrap()
        .children_by_field_name("name", &mut cursor)
        .map(|c| AstEnumField {
            name: c.utf8_text(source).unwrap().to_string(),
            typ: (),
            doc_comments: convert_doc_comments(c, source, source_span),
            span: span_from_node(source_span, c),
        })
        .collect::<Vec<_>>();

    AstEnum {
        name: text_child(node, source, "name"),
        fields,
        doc_comments: convert_doc_comments(node, source, source_span),
        span: span_from_node(source_span, node),
    }
}

fn convert_doc_comments(node: Node, source: &[u8], _source_span: Span) -> Option<String> {
    node.child_by_field_name("doc_comment")
        .map(|c| doc_comment_to_string(c, source))
}

fn convert_declaration_doc_comments(
    node: Node,
    source: &[u8],
    _source_span: Span,
) -> Option<String> {
    let first_child = node.child(0).unwrap();

    if first_child.kind() == "declaration_doc_comment" {
        Some(doc_comment_to_string(first_child, source))
    } else {
        None
    }
}

fn doc_comment_to_string(node: Node, source: &[u8]) -> String {
    let mut cursor = node.walk();
    node.children_by_field_name("doc_line", &mut cursor)
        .map(|c| c.utf8_text(source).unwrap().trim().to_string())
        .collect::<Vec<_>>()
        .join("\n")
}

fn convert_module(
    node: Node,
    source: &[u8],
    source_span: Span,
    filepath: &Path,
) -> AstModule<Untyped> {
    fn matching_nodes<'a, 'b>(
        node: Node<'a>,
        cursor: &'b mut TreeCursor<'a>,
        kind: &'static str,
    ) -> impl Iterator<Item = Node<'a>> + 'b {
        node.child_by_field_name("body")
            .unwrap()
            .children_by_field_name("field", cursor)
            .map(|n| n.child(0).unwrap())
            .filter(move |node| node.kind() == kind)
    }

    let annotations = node
        .children_by_field_name("annotation", &mut node.walk())
        .map(|c| convert_annotation(c, source, source_span))
        .collect::<Vec<_>>();

    let mut types: Vec<_> = matching_nodes(node, &mut node.walk(), "type")
        .map(|n| convert_model(n, source, source_span, AstModelKind::Type))
        .collect();
    let fragments: Vec<_> = matching_nodes(node, &mut node.walk(), "fragment")
        .map(|n| convert_model(n, source, source_span, AstModelKind::Fragment))
        .collect();
    let enums: Vec<_> = matching_nodes(node, &mut node.walk(), "enum")
        .map(|n| convert_enum(n, source, source_span))
        .collect();

    types.extend(fragments);

    AstModule {
        name: text_child(node, source, "name"),
        types,
        enums,
        methods: matching_nodes(node, &mut node.walk(), "module_method")
            .map(|n| convert_module_method(n, source, source_span))
            .collect(),
        interceptors: matching_nodes(node, &mut node.walk(), "interceptor")
            .map(|n| convert_interceptor(n, source, source_span))
            .collect(),
        annotations,
        base_exofile: filepath.into(),
        doc_comments: convert_doc_comments(node, source, source_span),
        span: span_from_node(source_span, node),
    }
}

fn convert_module_method(node: Node, source: &[u8], source_span: Span) -> AstMethod<Untyped> {
    let mut cursor = node.walk();

    AstMethod {
        name: text_child(node, source, "name"),
        typ: node
            .child_by_field_name("method_type")
            .unwrap()
            .utf8_text(source)
            .unwrap()
            .try_into()
            .unwrap(),
        arguments: node
            .children_by_field_name("args", &mut cursor)
            .map(|c| convert_argument(c, source, source_span))
            .collect(),
        return_type: node
            .child_by_field_name("return_type")
            .map(|c| convert_type(c, source, source_span))
            .unwrap(),
        is_exported: node.child_by_field_name("is_exported").is_some(),
        annotations: node
            .children_by_field_name("annotation", &mut cursor)
            .map(|c| convert_annotation(c, source, source_span))
            .collect(),
        doc_comments: convert_doc_comments(node, source, source_span),
        span: span_from_node(source_span, node),
    }
}

fn convert_interceptor(node: Node, source: &[u8], source_span: Span) -> AstInterceptor<Untyped> {
    let mut cursor = node.walk();

    AstInterceptor {
        name: text_child(node, source, "name"),
        arguments: node
            .children_by_field_name("args", &mut cursor)
            .map(|c| convert_argument(c, source, source_span))
            .collect(),
        annotations: node
            .children_by_field_name("annotation", &mut cursor)
            .map(|c| convert_annotation(c, source, source_span))
            .collect(),
        doc_comments: convert_doc_comments(node, source, source_span),
        span: span_from_node(source_span, node),
    }
}

fn convert_fields_and_fragments(
    node: Node,
    source: &[u8],
    source_span: Span,
) -> (Vec<AstField<Untyped>>, Vec<AstFragmentReference<Untyped>>) {
    let mut cursor = node.walk();

    let fields = node
        .children_by_field_name("field", &mut cursor)
        .map(|c| convert_field(c, source, source_span))
        .collect();
    let fragment_references = node
        .children_by_field_name("fragment_reference", &mut cursor)
        .map(|c| AstFragmentReference {
            name: text_child(c, source, "name"),
            typ: (),
            doc_comments: convert_doc_comments(c, source, source_span),
            span: span_from_node(source_span, c),
        })
        .collect();

    (fields, fragment_references)
}

fn convert_field(node: Node, source: &[u8], source_span: Span) -> AstField<Untyped> {
    assert!(node.kind() == "field");

    let mut cursor = node.walk();

    AstField {
        name: text_child(node, source, "name"),
        typ: convert_type(
            node.child_by_field_name("field_type").unwrap(),
            source,
            source_span,
        ),
        default_value: node
            .child_by_field_name("default_value")
            .map(|node| convert_field_default_value(node, source, source_span)),
        annotations: node
            .children_by_field_name("annotation", &mut cursor)
            .map(|c| convert_annotation(c, source, source_span))
            .collect(),
        doc_comments: convert_doc_comments(node, source, source_span),
        span: span_from_node(source_span, node),
    }
}

fn convert_field_default_value(
    node: Node,
    source: &[u8],
    source_span: Span,
) -> AstFieldDefault<Untyped> {
    let kind = {
        if let Some(node) = node.child_by_field_name("default_value_concrete") {
            AstFieldDefaultKind::Value(convert_expression(node, source, source_span))
        } else if let Some(node_fn) = node.child_by_field_name("default_value_fn") {
            let mut cursor = node.walk();

            let fn_name = node_fn.utf8_text(source).unwrap().to_string();
            let args = node
                .children_by_field_name("default_value_fn_args", &mut cursor)
                .map(|node_arg| convert_expression(node_arg, source, source_span))
                .collect();

            AstFieldDefaultKind::Function(fn_name, args)
        } else {
            panic!("no valid default field")
        }
    };

    AstFieldDefault {
        kind,
        span: span_from_node(source_span, node),
    }
}

fn convert_argument(node: Node, source: &[u8], source_span: Span) -> AstArgument<Untyped> {
    assert!(node.kind() == "argument");

    let mut cursor = node.walk();

    AstArgument {
        name: text_child(node, source, "name"),
        typ: convert_type(
            node.child_by_field_name("argument_type").unwrap(),
            source,
            source_span,
        ),
        annotations: node
            .children_by_field_name("annotation", &mut cursor)
            .map(|c| convert_annotation(c, source, source_span))
            .collect(),
    }
}

fn convert_type(node: Node, source: &[u8], source_span: Span) -> AstFieldType<Untyped> {
    assert_eq!(node.kind(), "field_type");
    let first_child = node.child(0).unwrap();
    let mut cursor = node.walk();

    match first_child.kind() {
        "field_term" => {
            let module_name = first_child
                .child_by_field_name("module")
                .map(|n| n.utf8_text(source).unwrap().to_string());
            let type_name = first_child
                .child_by_field_name("name")
                .map(|n| n.utf8_text(source).unwrap().to_string())
                .unwrap();

            AstFieldType::Plain(
                module_name,
                type_name,
                node.children_by_field_name("type_param", &mut cursor)
                    .map(|p| convert_type(p, source, source_span))
                    .collect(),
                (),
                span_from_node(source_span, first_child),
            )
        }
        "optional_field_type" => AstFieldType::Optional(Box::new(convert_type(
            first_child.child_by_field_name("inner").unwrap(),
            source,
            source_span,
        ))),
        o => panic!("unsupported declaration kind: {o}"),
    }
}

fn convert_annotation(node: Node, source: &[u8], source_span: Span) -> AstAnnotation<Untyped> {
    assert_eq!(node.kind(), "annotation");

    let name_node = node.child_by_field_name("name").unwrap();

    AstAnnotation {
        name: name_node.utf8_text(source).unwrap().to_string(),
        params: match node.child_by_field_name("params") {
            Some(node) => convert_annotation_params(node, source, source_span),
            None => AstAnnotationParams::None,
        },
        span: span_from_node(source_span, name_node),
    }
}

fn convert_annotation_params(
    node: Node,
    source: &[u8],
    source_span: Span,
) -> AstAnnotationParams<Untyped> {
    assert_eq!(node.kind(), "annotation_params");
    let mut cursor = node.walk();
    let first_child = node.child(0).unwrap();

    match first_child.kind() {
        "annotation_multiple_params" => {
            let (exprs, spans): (Vec<_>, Vec<_>) = first_child
                .children_by_field_name("exprs", &mut cursor)
                .map(|node| {
                    let expr = convert_expression(node, source, source_span);
                    let span = span_from_node(source_span, node);

                    (expr, span)
                })
                .unzip();

            let first_child_span = span_from_node(source_span, first_child);

            if exprs.len() == 1 {
                AstAnnotationParams::Single(exprs[0].clone(), first_child_span)
            } else {
                // try as a string list
                let string_list = exprs
                    .iter()
                    .map(|expr| match expr {
                        AstExpr::StringLiteral(string, _) => string.clone(),
                        _ => panic!("Only string literals are allowed in a list currently"),
                    })
                    .collect();

                AstAnnotationParams::Single(
                    AstExpr::StringList(string_list, spans),
                    first_child_span,
                )
            }
        }
        "annotation_map_params" => {
            let params = first_child
                .children_by_field_name("param", &mut cursor)
                .map(|p| (text_child(p, source, "name"), p))
                .collect::<Vec<_>>();

            let exprs = params
                .iter()
                .map(|(name, p)| {
                    (
                        name.clone(),
                        convert_expression(
                            p.child_by_field_name("expr").unwrap(),
                            source,
                            source_span,
                        ),
                    )
                })
                .collect();

            let mut spans: HashMap<String, Vec<Span>> = HashMap::new();

            for (name, node) in &params {
                let span = span_from_node(source_span, *node);
                match spans.get_mut(name) {
                    Some(spans) => spans.push(span),
                    None => {
                        spans.insert(name.clone(), vec![span]);
                    }
                }
            }

            AstAnnotationParams::Map(exprs, spans)
        }
        o => panic!("unsupported annotation params kind: {o}"),
    }
}

fn convert_literal(node: Node, source: &[u8], source_span: Span) -> AstExpr<Untyped> {
    let first_child = node.child(0).unwrap();

    match first_child.kind() {
        "literal_number" => AstExpr::NumberLiteral(
            first_child
                .child_by_field_name("value")
                .unwrap()
                .utf8_text(source)
                .unwrap()
                .to_string(),
            span_from_node(
                source_span,
                first_child.child_by_field_name("value").unwrap(),
            ),
        ),
        "literal_str" => AstExpr::StringLiteral(
            text_child(first_child, source, "value"),
            span_from_node(
                source_span,
                first_child.child_by_field_name("value").unwrap(),
            ),
        ),
        "literal_boolean" => {
            let value = first_child.child(0).unwrap().utf8_text(source).unwrap();
            AstExpr::BooleanLiteral(value == "true", source_span)
        }
        "literal_null" => AstExpr::NullLiteral(span_from_node(source_span, first_child)),
        _ => panic!("Unsupported literal type {:?}", first_child.kind()),
    }
}

fn convert_object_literal(node: Node, source: &[u8], source_span: Span) -> AstExpr<Untyped> {
    assert_eq!(node.kind(), "object_literal");
    let mut cursor = node.walk();
    let mut map = HashMap::new();

    for pair_node in node.children(&mut cursor) {
        if pair_node.kind() == "object_pair" {
            let key_node = pair_node.child_by_field_name("key").unwrap();
            let value_node = pair_node.child_by_field_name("value").unwrap();

            let key = match key_node.kind() {
                "term" => key_node.utf8_text(source).unwrap().to_string(),
                "literal_str" => text_child(key_node, source, "value"),
                _ => panic!("Unsupported key type: {}", key_node.kind()),
            };

            let value = convert_expression(value_node, source, source_span);
            map.insert(key, value);
        }
    }

    AstExpr::ObjectLiteral(map, span_from_node(source_span, node))
}

fn convert_expression(node: Node, source: &[u8], source_span: Span) -> AstExpr<Untyped> {
    assert_eq!(node.kind(), "expression");
    let first_child = node.child(0).unwrap();

    match first_child.kind() {
        "literal" => convert_literal(first_child, source, source_span),
        "logical_op" => AstExpr::LogicalOp(convert_logical_op(first_child, source, source_span)),
        "relational_op" => {
            AstExpr::RelationalOp(convert_relational_op(first_child, source, source_span))
        }
        "selection" => AstExpr::FieldSelection(convert_selection(first_child, source, source_span)),
        "parenthetical" => {
            let expression = first_child.child_by_field_name("expression").unwrap();
            convert_expression(expression, source, source_span)
        }
        "object_literal" => convert_object_literal(first_child, source, source_span),
        o => panic!("unsupported expression kind: {o}"),
    }
}

fn convert_logical_op(node: Node, source: &[u8], source_span: Span) -> LogicalOp<Untyped> {
    assert_eq!(node.kind(), "logical_op");
    let first_child = node.child(0).unwrap();

    match first_child.kind() {
        "logical_or" => LogicalOp::Or(
            Box::new(convert_expression(
                first_child.child_by_field_name("left").unwrap(),
                source,
                source_span,
            )),
            Box::new(convert_expression(
                first_child.child_by_field_name("right").unwrap(),
                source,
                source_span,
            )),
            source_span,
            (),
        ),
        "logical_and" => LogicalOp::And(
            Box::new(convert_expression(
                first_child.child_by_field_name("left").unwrap(),
                source,
                source_span,
            )),
            Box::new(convert_expression(
                first_child.child_by_field_name("right").unwrap(),
                source,
                source_span,
            )),
            source_span,
            (),
        ),
        "logical_not" => LogicalOp::Not(
            Box::new(convert_expression(
                first_child.child_by_field_name("value").unwrap(),
                source,
                source_span,
            )),
            span_from_node(source_span, first_child),
            (),
        ),
        o => panic!("unsupported logical op kind: {o}"),
    }
}

fn convert_relational_op(node: Node, source: &[u8], source_span: Span) -> RelationalOp<Untyped> {
    assert_eq!(node.kind(), "relational_op");
    let first_child = node.child(0).unwrap();

    let left_expr = Box::new(convert_expression(
        first_child.child_by_field_name("left").unwrap(),
        source,
        source_span,
    ));
    let right_expr = Box::new(convert_expression(
        first_child.child_by_field_name("right").unwrap(),
        source,
        source_span,
    ));

    match first_child.kind() {
        "relational_eq" => RelationalOp::Eq(left_expr, right_expr, ()),
        "relational_neq" => RelationalOp::Neq(left_expr, right_expr, ()),
        "relational_lt" => RelationalOp::Lt(left_expr, right_expr, ()),
        "relational_lte" => RelationalOp::Lte(left_expr, right_expr, ()),
        "relational_gt" => RelationalOp::Gt(left_expr, right_expr, ()),
        "relational_gte" => RelationalOp::Gte(left_expr, right_expr, ()),
        "relational_in" => RelationalOp::In(left_expr, right_expr, ()),
        o => panic!("unsupported relational op kind: {o}"),
    }
}

fn convert_selection(node: Node, source: &[u8], source_span: Span) -> FieldSelection<Untyped> {
    assert_eq!(node.kind(), "selection");
    let first_child = node.child(0).unwrap();

    match first_child.kind() {
        "selection_select" => FieldSelection::Select(
            Box::new(convert_selection(
                first_child.child_by_field_name("prefix").unwrap(),
                source,
                source_span,
            )),
            convert_selection_elem(
                first_child
                    .child_by_field_name("selection_element")
                    .unwrap(),
                source,
                source_span,
            ),
            span_from_node(source_span, first_child),
            (),
        ),
        "term" => FieldSelection::Single(
            FieldSelectionElement::Identifier(
                first_child.utf8_text(source).unwrap().to_string(),
                span_from_node(source_span, first_child),
                (),
            ),
            (),
        ),
        o => panic!("unsupported logical op kind: {o}"),
    }
}

fn convert_selection_elem(
    node: Node,
    source: &[u8],
    source_span: Span,
) -> FieldSelectionElement<Untyped> {
    assert_eq!(node.kind(), "selection_element");
    let first_child = node.child(0).unwrap();

    match first_child.kind() {
        "term" => FieldSelectionElement::Identifier(
            first_child.utf8_text(source).unwrap().to_string(),
            span_from_node(source_span, first_child),
            (),
        ),
        "func_call" => {
            let name_field = first_child.child_by_field_name("name").unwrap();
            let name = name_field.utf8_text(source).unwrap().to_string();

            let hof_args = first_child.child_by_field_name("hof_args");

            match hof_args {
                Some(hof_args) => {
                    let param_name_field = hof_args.child_by_field_name("param_name").unwrap();
                    let param_name = param_name_field.utf8_text(source).unwrap().to_string();

                    let expr_field = hof_args.child_by_field_name("expr").unwrap();
                    let expr = convert_expression(expr_field, source, source_span);
                    FieldSelectionElement::HofCall {
                        span: span_from_node(source_span, hof_args),
                        name: Identifier(name, span_from_node(source_span, name_field)),
                        param_name: Identifier(
                            param_name,
                            span_from_node(source_span, param_name_field),
                        ),
                        expr: Box::new(expr),
                        typ: (),
                    }
                }
                None => {
                    let mut cursor = first_child.walk();

                    let params_child =
                        first_child.children_by_field_name("normal_param", &mut cursor);

                    let params: Vec<_> = params_child
                        .flat_map(|c| {
                            if c.kind() == "literal" {
                                Some(convert_literal(c, source, source_span))
                            } else {
                                None
                            }
                        })
                        .collect();

                    FieldSelectionElement::NormalCall {
                        span: span_from_node(source_span, first_child),
                        name: Identifier(name, span_from_node(source_span, name_field)),
                        params,
                        typ: (),
                    }
                }
            }
        }
        o => panic!("unsupported selection element kind: {o}"),
    }
}

fn text_child(node: Node, source: &[u8], child_name: &str) -> String {
    node.child_by_field_name(child_name)
        .unwrap()
        .utf8_text(source)
        .unwrap()
        .to_string()
}

#[cfg(test)]
mod tests {
    use codemap::CodeMap;
    use multiplatform_test::multiplatform_test;

    use super::*;

    // Due to a change in insta version 1.12, test names (hence the snapshot names) get derived
    // from the surrounding method, so we must use a macro instead of a helper function.
    macro_rules! parsing_test {
        ($src:literal, $fn_name:expr) => {
            let mut codemap = CodeMap::new();
            let file_span = codemap
                .add_file("input.exo".to_string(), $src.to_string())
                .span;
            let parsed = parse($src).unwrap();

            insta::with_settings!({prepend_module_to_snapshot => false}, {
                #[cfg(target_family = "wasm")]
                {
                    let to_check = convert_root(
                        parsed.root_node(),
                        $src.as_bytes(),
                        file_span,
                        Path::new("input.exo")
                    )
                    .unwrap();

                    let expected = include_str!(concat!("./snapshots/", $fn_name, ".snap"));
                    let split_expected = expected.split("---\n").skip(2).collect::<Vec<&str>>().join("---");
                    let serialized = insta::_macro_support::serialize_value(
                        &to_check,
                        insta::_macro_support::SerializationFormat::Yaml,
                    );
                    assert_eq!(split_expected, serialized);
                }

                #[cfg(not(target_family = "wasm"))]
                {

                    insta::assert_yaml_snapshot!(convert_root(
                        parsed.root_node(),
                        $src.as_bytes(),
                        file_span,
                        Path::new("input.exo")
                    ).unwrap())
                }
            })
        };
    }

    #[multiplatform_test]
    fn expression_precedence() {
        parsing_test!(
            r#"
            @postgres
            module TestModule{            
                type Foo {
                    @column("custom_column") @access(!self.role == "role_admin" || self.role == "role_superuser")
                    bar: Baz
                }
            }
        "#,
            "expression_precedence"
        );
    }

    #[multiplatform_test]
    fn logical_op_precedence() {
        // Should parse as `a || (b && c)`
        parsing_test!(
            r#"
            @postgres
            module TestModule {
                @access(a || b && c)
                type Foo {
                }
            }
        "#,
            "logical_op_precedence"
        );
    }

    #[multiplatform_test]
    fn logical_not_precedence_logical() {
        // Should parse as `(!a) || b`
        parsing_test!(
            r#"
            @postgres
            module TestModule {       
                @access(!a || b)
                type Foo {
                }
            }
        "#,
            "logical_not_precedence_logical"
        );
    }

    #[multiplatform_test]
    fn logical_not_precedence_relational() {
        // Should parse as `(!a) == b`
        parsing_test!(
            r#"
            @postgres
            module TestModule{            
                @access(!a == b)
                type Foo {
                }
            }
        "#,
            "logical_not_precedence_relational"
        );
    }

    #[multiplatform_test]
    fn bb_schema() {
        parsing_test!(
            r#"
            @postgres
            module TestModule{
                // a short comment
                @table("concerts")
                type Concert {
                    @pk id: Int = autoIncrement()
                    title: String // a comment
                    // another comment
                    @column("venueid") venue: Venue 
                    /*
                    not_a_field: Int
                    */
                }

                /*
                a multiline comment
                */
                @table("venues")
                type Venue {
                    @pk id: Int = autoIncrement()
                    name: String
                    /*here */ @column("venueid") /* and here */ concerts: Set<Concert /* here too! */> 
                }
            }
        "#,
            "bb_schema"
        );
    }

    #[multiplatform_test]
    fn context_schema() {
        parsing_test!(
            r#"
            context AuthUser {
                @jwt("sub") id: Int 
                @jwt roles: Array<String> 
            }
        "#,
            "context_schema"
        );
    }

    #[multiplatform_test]
    fn access_control_function_without_paren() {
        parsing_test!(
            r#"
            @postgres
            module TestModule {
                @access(self.concerts.some(c => c.id == 1))
                type Venue {
                    concerts: Set<Concert>?
                }

                type Concert {
                    @pk id: Int = autoIncrement()
                    venue: Venue
                }
            }
        "#,
            "access_control_function_without_paren"
        );
    }

    #[multiplatform_test]
    fn access_control_function_with_paren() {
        parsing_test!(
            r#"
            @postgres
            module TestModule {
                @access(self.concerts.some((c) => c.id == 1))
                type Venue {
                    concerts: Set<Concert>?
                }

                type Concert {
                    @pk id: Int = autoIncrement()
                    venue: Venue
                }
            }
        "#,
            "access_control_function_with_paren"
        );
    }

    #[multiplatform_test]
    fn doc_comments_triple_slash() {
        parsing_test!(
            r#"
            @postgres
            /// Todo database module line 1
            /// Todo database module line 2
            module TestModule {
                /// Todo database type line 1
                /// Todo database type line 2
                type Todo {
                    /// Todo database field id line 1
                    /// Todo database field id line 2
                    id: Int
                    /// Todo database field title line 1
                    /// Todo database field title line 2
                    title: String
                }

                /// Todo database method line 1
                /// Todo database method line 2
                query getTodo(id: Int): Todo

                /// Todo database interceptor line 1
                /// Todo database interceptor line 2
                interceptor getTodoInterceptor(id: Int)
            }
            "#,
            "doc_comments_triple_slash"
        );
    }

    #[multiplatform_test]
    fn doc_comments_block_comment() {
        parsing_test!(
            r#"
            @postgres
            /**
             *  Todo database module line 1
             *  Todo database module line 2
             */
            module TestModule {
                /**
                 *  Todo database type line 1
                 *  Todo database type line 2
                 */
                type Todo {
                    /**
                     *  Todo database field id line 1
                     *  Todo database field id line 2
                     */
                    id: Int
                    /**
                     *  Todo database field title line 1
                     *  Todo database field title line 2
                     */
                    title: String
                }

                /**
                 *  Todo database method line 1
                 *  Todo database method line 2
                 */
                query getTodo(id: Int): Todo

                /**
                 *  Todo database interceptor line 1
                 *  Todo database interceptor line 2
                 */
                interceptor getTodoInterceptor(id: Int)
            }
            "#,
            "doc_comments_block_comment"
        );
    }
}
