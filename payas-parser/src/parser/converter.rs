use tree_sitter::{Node, Tree};

use crate::ast::ast_types::*;
use super::sitter_ffi;

struct AstAnnotation {
    name: String,
    params: Vec<AstExpr>
}

pub fn parse(input: &str) -> Option<Tree> {
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(sitter_ffi::language()).unwrap();
    let tree = parser.parse(input, None);
    tree
}

pub fn convert_root(node: Node, source: &[u8]) -> AstSystem {
    assert_eq!(node.kind(), "source_file");
    if node.has_error() {
        panic!("tree has an error");
    }
    let mut cursor = node.walk();
    AstSystem {
        types: node.children(&mut cursor).map(|c| convert_declaration(c, source)).collect()
    }
}

pub fn convert_declaration(node: Node, source: &[u8]) -> AstType {
    assert_eq!(node.kind(), "declaration");
    let first_child = node.child(0).unwrap();

    match first_child.kind() {
        "model" => convert_model(first_child, source),
        o => panic!("unsupported declaration kind: {}", o)
    }
}

pub fn convert_model(node: Node, source: &[u8]) -> AstType {
    assert_eq!(node.kind(), "model");

    AstType {
        name: node.child_by_field_name("name").unwrap().utf8_text(source).unwrap().to_string(),
        kind: AstTypeKind::Composite {
            fields: convert_fields(node.child_by_field_name("body").unwrap(), source),
            table_name: None
        }
    }
}

pub fn convert_fields(node: Node, source: &[u8]) -> Vec<AstField> {
    let mut cursor = node.walk();
    node.children_by_field_name("field", &mut cursor).map(|c| convert_field(c, source)).collect()
}

pub fn convert_field(node: Node, source: &[u8]) -> AstField {
    assert_eq!(node.kind(), "field");

    let mut cursor = node.walk();
    let annotations: Vec<_> = node.children_by_field_name("annotation", &mut cursor).map(|c| convert_annotation(c, source)).collect();

    let mut column_name: Option<String> = None;
    let mut auth: Option<AstExpr> = None;

    for annotation in annotations {
        match annotation.name.as_str() {
            "column" => {
                if column_name.is_some() {
                    panic!("duplicate column annotations")
                } else {
                    assert!(annotation.params.len() == 1);
                    if let AstExpr::StringLiteral(ref value) = annotation.params[0] { 
                        column_name = Some(value.clone());
                    } else {
                        panic!("expected literal string")
                    };
                }
            }

            "auth" => {
                if auth.is_some() {
                    panic!("duplicate auth annotations")
                } else {
                    assert!(annotation.params.len() == 1);
                    auth = Some(annotation.params[0].clone());
                }
            }

            o => panic!("unexpected annotation: {}", o)
        }
    }

    AstField {
        name: node.child_by_field_name("name").unwrap().utf8_text(source).unwrap().to_string(),
        typ: AstFieldType::Plain(
            AstType {
                name: node.child_by_field_name("type").unwrap().utf8_text(source).unwrap().to_string(),
                kind: AstTypeKind::Other
            }
        ),
        relation: AstRelation::Other {
            optional: false
        },
        column_name: column_name,
        auth: auth
    }
}

fn convert_annotation(node: Node, source: &[u8]) -> AstAnnotation {
    assert_eq!(node.kind(), "annotation");
    let mut cursor = node.walk();
    AstAnnotation {
        name: node.child_by_field_name("name").unwrap().utf8_text(source).unwrap().to_string(),
        params: node.children_by_field_name("param", &mut cursor).map(|c| convert_expression(c, source)).collect()
    }
}

fn convert_expression(node: Node, source: &[u8]) -> AstExpr {
    assert_eq!(node.kind(), "expression");
    let first_child = node.child(0).unwrap();

    match first_child.kind() {
        "literal_str" => AstExpr::StringLiteral(
            first_child.child_by_field_name("value").unwrap().utf8_text(source).unwrap().to_string()
        ),
        "logical_op" => AstExpr::LogicalOp(
            convert_logical_op(first_child, source)
        ),
        "relational_op" => AstExpr::RelationalOp(
            convert_relational_op(first_child, source)
        ),
        "selection" => AstExpr::FieldSelection(
            convert_selection(first_child, source)
        ),
        o => panic!("unsupported expression kind: {}", o)
    }
}

fn convert_logical_op(node: Node, source: &[u8]) -> LogicalOp {
    assert_eq!(node.kind(), "logical_op");
    let first_child = node.child(0).unwrap();

    match first_child.kind() {
        "logical_or" => LogicalOp::Or(
            Box::new(convert_expression(first_child.child_by_field_name("left").unwrap(), source)),
            Box::new(convert_expression(first_child.child_by_field_name("right").unwrap(), source))
        ),
        "logical_and" => LogicalOp::And(
            Box::new(convert_expression(first_child.child_by_field_name("left").unwrap(), source)),
            Box::new(convert_expression(first_child.child_by_field_name("right").unwrap(), source))
        ),
        "logical_not" => LogicalOp::Not(
            Box::new(convert_expression(first_child.child_by_field_name("value").unwrap(), source))
        ),
        o => panic!("unsupported logical op kind: {}", o)
    }
}

fn convert_relational_op(node: Node, source: &[u8]) -> RelationalOp {
    assert_eq!(node.kind(), "relational_op");
    let first_child = node.child(0).unwrap();

    match first_child.kind() {
        "relational_eq" => RelationalOp::Eq(
            Box::new(convert_expression(first_child.child_by_field_name("left").unwrap(), source)),
            Box::new(convert_expression(first_child.child_by_field_name("right").unwrap(), source))
        ),
        "relational_neq" => RelationalOp::Neq(
            Box::new(convert_expression(first_child.child_by_field_name("left").unwrap(), source)),
            Box::new(convert_expression(first_child.child_by_field_name("right").unwrap(), source))
        ),
        "relational_lt" => RelationalOp::Lt(
            Box::new(convert_expression(first_child.child_by_field_name("left").unwrap(), source)),
            Box::new(convert_expression(first_child.child_by_field_name("right").unwrap(), source))
        ),
        "relational_lte" => RelationalOp::Lte(
            Box::new(convert_expression(first_child.child_by_field_name("left").unwrap(), source)),
            Box::new(convert_expression(first_child.child_by_field_name("right").unwrap(), source))
        ),
        "relational_gt" => RelationalOp::Gt(
            Box::new(convert_expression(first_child.child_by_field_name("left").unwrap(), source)),
            Box::new(convert_expression(first_child.child_by_field_name("right").unwrap(), source))
        ),
        "relational_gte" => RelationalOp::Gte(
            Box::new(convert_expression(first_child.child_by_field_name("left").unwrap(), source)),
            Box::new(convert_expression(first_child.child_by_field_name("right").unwrap(), source))
        ),
        o => panic!("unsupported relational op kind: {}", o)
    }
}

fn convert_selection(node: Node, source: &[u8]) -> FieldSelection {
    assert_eq!(node.kind(), "selection");
    let first_child = node.child(0).unwrap();

    match first_child.kind() {
        "selection_select" => FieldSelection::Select(
            Box::new(convert_selection(first_child.child_by_field_name("prefix").unwrap(), source)),
            Identifier(
                first_child.child_by_field_name("term").unwrap().utf8_text(source).unwrap().to_string()
            )
        ),
        "term" => FieldSelection::Single(
            Identifier(
                first_child.utf8_text(source).unwrap().to_string()
            )
        ),
        o => panic!("unsupported logical op kind: {}", o)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expression_precedence() {
        let src = r#"
        model Foo {
            bar: Baz @column("custom_column") @auth(!self.role == "role_admin" || self.role == "role_superuser")
        }
        "#;
        let parsed = parse(src).unwrap();
        dbg!(parsed.root_node().to_sexp());
        insta::assert_yaml_snapshot!(convert_root(parsed.root_node(), src.as_bytes()));
    }

    #[test]
    fn bb_schema() {
        let src = r#"
        @table("concerts")
        model Concert {
          id: Int @pk @autoincrement
          title: String
          venue: Venue @column("venueid")
        }

        @table("venues")
        model Venue {
          id: Int @pk @autoincrement
          name: String
          concerts: [Concert] @column("venueid")
        }
        "#;
        let parsed = parse(src).unwrap();
        dbg!(parsed.root_node().to_sexp());
        insta::assert_yaml_snapshot!(convert_root(parsed.root_node(), src.as_bytes()));
    }
}
