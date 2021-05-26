use tree_sitter::{Node, Tree};

use crate::ast::ast_types::*;

mod sitter_ffi;

pub fn parse(input: &str) -> Option<Tree> {
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(sitter_ffi::language()).unwrap();
    let tree = parser.parse(input, None);
    tree
}

pub fn convert_root(node: Node, source: &[u8]) -> AstSystem {
    assert_eq!(node.kind(), "source_file");
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
            fields: vec![],
            table_name: None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic() {
        let src = r#"
        model Foo {
            bar: Baz @auth(foo.bar | bar.baz)
        }
        "#;
        let parsed = parse(src).unwrap();
        insta::assert_yaml_snapshot!(convert_root(parsed.root_node(), src.as_bytes()));
    }
}
