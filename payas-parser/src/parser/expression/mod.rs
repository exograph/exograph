use tree_sitter::{Node, Tree};

mod sitter_ffi;

pub fn parse(input: &str) -> Option<Tree> {
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(sitter_ffi::language()).unwrap();
    let tree = parser.parse(input, None);
    tree
}

// #[derive(Debug, Clone, PartialEq)]
// pub enum LogicalOp<'a> {
//     Not(Box<AstExpr<'a>>),
//     And(Box<AstExpr<'a>>, Box<AstExpr<'a>>),
//     Or(Box<AstExpr<'a>>, Box<AstExpr<'a>>),
// }

// pub fn convert_node(node: Node) -> LogicalOp {
//     match node.kind() {
//         "source_file" => node.children().map(convert_node),
//     }
//     node.child_by_field_name("field_name").unwrap()
// }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic() {
        dbg!(parse(
            r#"
        model Foo {
            bar: Baz @auth(foo.bar | bar.baz)
        }
        "#
        )
        .unwrap()
        .root_node()
        .to_sexp());
    }
}
