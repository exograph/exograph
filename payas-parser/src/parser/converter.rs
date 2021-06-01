use tree_sitter::{Node, Tree};

use super::sitter_ffi;
use crate::ast::ast_types::*;

pub fn parse(input: &str) -> Option<Tree> {
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(sitter_ffi::language()).unwrap();
    parser.parse(input, None)
}

pub fn convert_root(node: Node, source: &[u8]) -> AstSystem {
    assert_eq!(node.kind(), "source_file");
    if node.has_error() {
        dbg!(node.to_sexp());
        panic!("tree has an error");
    }
    let mut cursor = node.walk();
    AstSystem {
        models: node
            .children(&mut cursor)
            .map(|c| convert_declaration(c, source))
            .collect(),
    }
}

pub fn convert_declaration(node: Node, source: &[u8]) -> AstModel {
    assert_eq!(node.kind(), "declaration");
    let first_child = node.child(0).unwrap();

    match first_child.kind() {
        "model" => convert_model(first_child, source),
        o => panic!("unsupported declaration kind: {}", o),
    }
}

pub fn convert_model(node: Node, source: &[u8]) -> AstModel {
    assert_eq!(node.kind(), "model");

    let mut cursor = node.walk();

    AstModel {
        name: node
            .child_by_field_name("name")
            .unwrap()
            .utf8_text(source)
            .unwrap()
            .to_string(),
        fields: convert_fields(node.child_by_field_name("body").unwrap(), source),
        annotations: node
            .children_by_field_name("annotation", &mut cursor)
            .map(|c| convert_annotation(c, source))
            .collect(),
    }
}

pub fn convert_fields(node: Node, source: &[u8]) -> Vec<AstField> {
    let mut cursor = node.walk();
    node.children_by_field_name("field", &mut cursor)
        .map(|c| convert_field(c, source))
        .collect()
}

pub fn convert_field(node: Node, source: &[u8]) -> AstField {
    assert_eq!(node.kind(), "field");

    let mut cursor = node.walk();

    AstField {
        name: node
            .child_by_field_name("name")
            .unwrap()
            .utf8_text(source)
            .unwrap()
            .to_string(),
        typ: convert_type(node.child_by_field_name("type").unwrap(), source),
        annotations: node
            .children_by_field_name("annotation", &mut cursor)
            .map(|c| convert_annotation(c, source))
            .collect(),
    }
}

pub fn convert_type(node: Node, source: &[u8]) -> AstFieldType {
    assert_eq!(node.kind(), "type");
    let first_child = node.child(0).unwrap();

    match first_child.kind() {
        "term" => AstFieldType::Plain(first_child.utf8_text(source).unwrap().to_string()),
        "array_type" => AstFieldType::List(Box::new(convert_type(
            first_child.child_by_field_name("inner").unwrap(),
            source,
        ))),
        "optional_type" => AstFieldType::Optional(Box::new(convert_type(
            first_child.child_by_field_name("inner").unwrap(),
            source,
        ))),
        o => panic!("unsupported declaration kind: {}", o),
    }
}

fn convert_annotation(node: Node, source: &[u8]) -> AstAnnotation {
    assert_eq!(node.kind(), "annotation");
    let mut cursor = node.walk();
    AstAnnotation {
        name: node
            .child_by_field_name("name")
            .unwrap()
            .utf8_text(source)
            .unwrap()
            .to_string(),
        params: node
            .children_by_field_name("param", &mut cursor)
            .map(|c| convert_expression(c, source))
            .collect(),
    }
}

fn convert_expression(node: Node, source: &[u8]) -> AstExpr {
    assert_eq!(node.kind(), "expression");
    let first_child = node.child(0).unwrap();

    match first_child.kind() {
        "literal_str" => AstExpr::StringLiteral(
            first_child
                .child_by_field_name("value")
                .unwrap()
                .utf8_text(source)
                .unwrap()
                .to_string(),
        ),
        "logical_op" => AstExpr::LogicalOp(convert_logical_op(first_child, source)),
        "relational_op" => AstExpr::RelationalOp(convert_relational_op(first_child, source)),
        "selection" => AstExpr::FieldSelection(convert_selection(first_child, source)),
        o => panic!("unsupported expression kind: {}", o),
    }
}

fn convert_logical_op(node: Node, source: &[u8]) -> LogicalOp {
    assert_eq!(node.kind(), "logical_op");
    let first_child = node.child(0).unwrap();

    match first_child.kind() {
        "logical_or" => LogicalOp::Or(
            Box::new(convert_expression(
                first_child.child_by_field_name("left").unwrap(),
                source,
            )),
            Box::new(convert_expression(
                first_child.child_by_field_name("right").unwrap(),
                source,
            )),
        ),
        "logical_and" => LogicalOp::And(
            Box::new(convert_expression(
                first_child.child_by_field_name("left").unwrap(),
                source,
            )),
            Box::new(convert_expression(
                first_child.child_by_field_name("right").unwrap(),
                source,
            )),
        ),
        "logical_not" => LogicalOp::Not(Box::new(convert_expression(
            first_child.child_by_field_name("value").unwrap(),
            source,
        ))),
        o => panic!("unsupported logical op kind: {}", o),
    }
}

fn convert_relational_op(node: Node, source: &[u8]) -> RelationalOp {
    assert_eq!(node.kind(), "relational_op");
    let first_child = node.child(0).unwrap();

    match first_child.kind() {
        "relational_eq" => RelationalOp::Eq(
            Box::new(convert_expression(
                first_child.child_by_field_name("left").unwrap(),
                source,
            )),
            Box::new(convert_expression(
                first_child.child_by_field_name("right").unwrap(),
                source,
            )),
        ),
        "relational_neq" => RelationalOp::Neq(
            Box::new(convert_expression(
                first_child.child_by_field_name("left").unwrap(),
                source,
            )),
            Box::new(convert_expression(
                first_child.child_by_field_name("right").unwrap(),
                source,
            )),
        ),
        "relational_lt" => RelationalOp::Lt(
            Box::new(convert_expression(
                first_child.child_by_field_name("left").unwrap(),
                source,
            )),
            Box::new(convert_expression(
                first_child.child_by_field_name("right").unwrap(),
                source,
            )),
        ),
        "relational_lte" => RelationalOp::Lte(
            Box::new(convert_expression(
                first_child.child_by_field_name("left").unwrap(),
                source,
            )),
            Box::new(convert_expression(
                first_child.child_by_field_name("right").unwrap(),
                source,
            )),
        ),
        "relational_gt" => RelationalOp::Gt(
            Box::new(convert_expression(
                first_child.child_by_field_name("left").unwrap(),
                source,
            )),
            Box::new(convert_expression(
                first_child.child_by_field_name("right").unwrap(),
                source,
            )),
        ),
        "relational_gte" => RelationalOp::Gte(
            Box::new(convert_expression(
                first_child.child_by_field_name("left").unwrap(),
                source,
            )),
            Box::new(convert_expression(
                first_child.child_by_field_name("right").unwrap(),
                source,
            )),
        ),
        o => panic!("unsupported relational op kind: {}", o),
    }
}

fn convert_selection(node: Node, source: &[u8]) -> FieldSelection {
    assert_eq!(node.kind(), "selection");
    let first_child = node.child(0).unwrap();

    match first_child.kind() {
        "selection_select" => FieldSelection::Select(
            Box::new(convert_selection(
                first_child.child_by_field_name("prefix").unwrap(),
                source,
            )),
            Identifier(
                first_child
                    .child_by_field_name("term")
                    .unwrap()
                    .utf8_text(source)
                    .unwrap()
                    .to_string(),
            ),
        ),
        "term" => FieldSelection::Single(Identifier(
            first_child.utf8_text(source).unwrap().to_string(),
        )),
        o => panic!("unsupported logical op kind: {}", o),
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
        insta::assert_yaml_snapshot!(convert_root(parsed.root_node(), src.as_bytes()));
    }
}
