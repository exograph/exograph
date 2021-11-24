use std::convert::TryInto;
use std::{collections::HashMap, path::Path};

use codemap::Span;
use tree_sitter::{Node, Tree, TreeCursor};

use super::{sitter_ffi, span_from_node};
use crate::ast::ast_types::*;
use crate::error::ParserError;

pub fn parse(input: &str) -> Option<Tree> {
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(sitter_ffi::language()).unwrap();
    parser.parse(input, None)
}

pub fn convert_root(
    node: Node,
    source: &[u8],
    source_span: Span,
    filepath: &Path,
) -> Result<AstSystem<Untyped>, ParserError> {
    assert_eq!(node.kind(), "source_file");

    let mut cursor = node.walk();
    Ok(AstSystem {
        models: node
            .children(&mut cursor)
            .filter(|n| n.kind() == "declaration")
            .map(|c| convert_declaration_to_model(c, source, source_span))
            .flatten()
            .collect::<Vec<_>>(),
        services: node
            .children(&mut cursor)
            .filter(|n| n.kind() == "declaration")
            .map(|c| convert_declaration_to_service(c, source, source_span, filepath))
            .flatten()
            .collect::<Vec<_>>(),
    })
}

fn convert_declaration_to_model(
    node: Node,
    source: &[u8],
    source_span: Span,
) -> Option<AstModel<Untyped>> {
    assert_eq!(node.kind(), "declaration");
    let first_child = node.child(0).unwrap();

    if first_child.kind() == "model" {
        Some(convert_model(first_child, source, source_span))
    } else {
        None
    }
}

fn convert_declaration_to_service(
    node: Node,
    source: &[u8],
    source_span: Span,
    filepath: &Path,
) -> Option<AstService<Untyped>> {
    assert_eq!(node.kind(), "declaration");
    let first_child = node.child(0).unwrap();

    if first_child.kind() == "service" {
        let service = convert_service(first_child, source, source_span, filepath);
        Some(service)
    } else {
        None
    }
}

fn convert_model(node: Node, source: &[u8], source_span: Span) -> AstModel<Untyped> {
    assert_eq!(node.kind(), "model");

    let mut cursor = node.walk();

    let kind = node
        .child_by_field_name("kind")
        .unwrap()
        .utf8_text(source)
        .unwrap()
        .to_string();

    let kind = if kind == "model" {
        AstModelKind::Persistent
    } else if kind == "type" {
        AstModelKind::NonPersistent
    } else if kind == "input type" {
        AstModelKind::NonPersistentInput
    } else if kind == "context" {
        AstModelKind::Context
    } else {
        todo!()
    };

    AstModel {
        name: node
            .child_by_field_name("name")
            .unwrap()
            .utf8_text(source)
            .unwrap()
            .to_string(),
        kind,
        fields: convert_fields(
            node.child_by_field_name("body").unwrap(),
            source,
            source_span,
        ),
        annotations: node
            .children_by_field_name("annotation", &mut cursor)
            .map(|c| convert_annotation(c, source, source_span))
            .collect(),
    }
}

fn convert_service(
    node: Node,
    source: &[u8],
    source_span: Span,
    filepath: &Path,
) -> AstService<Untyped> {
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

    AstService {
        name: node
            .child_by_field_name("name")
            .unwrap()
            .utf8_text(source)
            .unwrap()
            .to_string(),
        models: matching_nodes(node, &mut node.walk(), "model")
            .map(|n| convert_model(n, source, source_span))
            .collect(),
        methods: matching_nodes(node, &mut node.walk(), "service_method")
            .map(|n| convert_service_method(n, source, source_span))
            .collect(),
        interceptors: matching_nodes(node, &mut node.walk(), "interceptor")
            .map(|n| convert_interceptor(n, source, source_span))
            .collect(),
        annotations: node
            .children_by_field_name("annotation", &mut node.walk())
            .map(|c| convert_annotation(c, source, source_span))
            .collect(),
        base_clayfile: filepath.into(),
    }
}

fn convert_service_method(node: Node, source: &[u8], source_span: Span) -> AstMethod<Untyped> {
    let mut cursor = node.walk();

    AstMethod {
        name: node
            .child_by_field_name("name")
            .unwrap()
            .utf8_text(source)
            .unwrap()
            .to_string(),
        typ: node
            .child_by_field_name("type")
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
    }
}

fn convert_interceptor(node: Node, source: &[u8], source_span: Span) -> AstInterceptor<Untyped> {
    let mut cursor = node.walk();

    AstInterceptor {
        name: node
            .child_by_field_name("name")
            .unwrap()
            .utf8_text(source)
            .unwrap()
            .to_string(),
        arguments: node
            .children_by_field_name("args", &mut cursor)
            .map(|c| convert_argument(c, source, source_span))
            .collect(),
        annotations: node
            .children_by_field_name("annotation", &mut cursor)
            .map(|c| convert_annotation(c, source, source_span))
            .collect(),
        span: span_from_node(source_span, node),
    }
}

fn convert_fields(node: Node, source: &[u8], source_span: Span) -> Vec<AstField<Untyped>> {
    let mut cursor = node.walk();
    node.children_by_field_name("field", &mut cursor)
        .map(|c| convert_field(c, source, source_span))
        .collect()
}

fn convert_field(node: Node, source: &[u8], source_span: Span) -> AstField<Untyped> {
    assert!(node.kind() == "field");

    let mut cursor = node.walk();

    AstField {
        name: node
            .child_by_field_name("name")
            .unwrap()
            .utf8_text(source)
            .unwrap()
            .to_string(),
        typ: convert_type(
            node.child_by_field_name("type").unwrap(),
            source,
            source_span,
        ),
        annotations: node
            .children_by_field_name("annotation", &mut cursor)
            .map(|c| convert_annotation(c, source, source_span))
            .collect(),
        span: span_from_node(source_span, node),
    }
}

// TODO: dedup
fn convert_argument(node: Node, source: &[u8], source_span: Span) -> AstArgument<Untyped> {
    assert!(node.kind() == "argument");

    let mut cursor = node.walk();

    AstArgument {
        name: node
            .child_by_field_name("name")
            .unwrap()
            .utf8_text(source)
            .unwrap()
            .to_string(),
        typ: convert_type(
            node.child_by_field_name("type").unwrap(),
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
    assert_eq!(node.kind(), "type");
    let first_child = node.child(0).unwrap();
    let mut cursor = node.walk();

    match first_child.kind() {
        "term" => AstFieldType::Plain(
            first_child.utf8_text(source).unwrap().to_string(),
            node.children_by_field_name("type_param", &mut cursor)
                .map(|p| convert_type(p, source, source_span))
                .collect(),
            (),
            span_from_node(source_span, first_child),
        ),
        "optional_type" => AstFieldType::Optional(Box::new(convert_type(
            first_child.child_by_field_name("inner").unwrap(),
            source,
            source_span,
        ))),
        o => panic!("unsupported declaration kind: {}", o),
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
        "expression" => AstAnnotationParams::Single(
            convert_expression(first_child, source, source_span),
            span_from_node(source_span, first_child),
        ),
        "annotation_map_params" => {
            let params = first_child
                .children_by_field_name("param", &mut cursor)
                .map(|p| {
                    (
                        p.child_by_field_name("name")
                            .unwrap()
                            .utf8_text(source)
                            .unwrap()
                            .to_string(),
                        p,
                    )
                })
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
        o => panic!("unsupported annotation params kind: {}", o),
    }
}

fn convert_expression(node: Node, source: &[u8], source_span: Span) -> AstExpr<Untyped> {
    assert_eq!(node.kind(), "expression");
    let first_child = node.child(0).unwrap();

    match first_child.kind() {
        "literal_number" => AstExpr::NumberLiteral(
            first_child
                .child_by_field_name("value")
                .unwrap()
                .utf8_text(source)
                .map(|s| s.parse::<i64>().unwrap())
                .unwrap(),
            span_from_node(
                source_span,
                first_child.child_by_field_name("value").unwrap(),
            ),
        ),
        "literal_str" => AstExpr::StringLiteral(
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
        "literal_boolean" => {
            let value = first_child.child(0).unwrap().utf8_text(source).unwrap();
            if value == "true" {
                AstExpr::BooleanLiteral(true, source_span)
            } else {
                AstExpr::BooleanLiteral(false, source_span)
            }
        }
        "logical_op" => AstExpr::LogicalOp(convert_logical_op(first_child, source, source_span)),
        "relational_op" => {
            AstExpr::RelationalOp(convert_relational_op(first_child, source, source_span))
        }
        "selection" => AstExpr::FieldSelection(convert_selection(first_child, source, source_span)),
        o => panic!("unsupported expression kind: {}", o),
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
        o => panic!("unsupported logical op kind: {}", o),
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
        o => panic!("unsupported relational op kind: {}", o),
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
            Identifier(
                first_child
                    .child_by_field_name("term")
                    .unwrap()
                    .utf8_text(source)
                    .unwrap()
                    .to_string(),
                span_from_node(
                    source_span,
                    first_child.child_by_field_name("term").unwrap(),
                ),
            ),
            span_from_node(source_span, first_child),
            (),
        ),
        "term" => FieldSelection::Single(
            Identifier(
                first_child.utf8_text(source).unwrap().to_string(),
                span_from_node(source_span, first_child),
            ),
            (),
        ),
        o => panic!("unsupported logical op kind: {}", o),
    }
}

#[cfg(test)]
mod tests {
    use codemap::CodeMap;

    use super::*;

    fn parsing_test(src: &str) {
        let mut codemap = CodeMap::new();
        let file_span = codemap
            .add_file("input.payas".to_string(), src.to_string())
            .span;
        let parsed = parse(src).unwrap();
        insta::assert_yaml_snapshot!(convert_root(
            parsed.root_node(),
            src.as_bytes(),
            file_span,
            Path::new("input.payas")
        )
        .unwrap());
    }

    #[test]
    fn expression_precedence() {
        parsing_test(
            r#"
        model Foo {
            bar: Baz @column("custom_column") @access(!self.role == "role_admin" || self.role == "role_superuser")
        }
        "#,
        );
    }

    #[test]
    fn bb_schema() {
        parsing_test(
            r#"
        // a short comment
        @table("concerts")
        model Concert {
          id: Int @pk @autoincrement
          title: String // a comment
          // another comment
          venue: Venue @column("venueid")
          /*
          not_a_field: Int
          */
        }

        /*
        a multiline comment
        */
        @table("venues")
        model Venue {
          id: Int @pk @autoincrement
          name: String
          concerts: Set<Concert /* here too! */> @column("venueid")
        }
        "#,
        );
    }

    #[test]
    fn context_schema() {
        parsing_test(
            r#"
        context AuthUser {
            id: Int @jwt("sub")
            roles: Array<String> @jwt
         }
        "#,
        );
    }
}
