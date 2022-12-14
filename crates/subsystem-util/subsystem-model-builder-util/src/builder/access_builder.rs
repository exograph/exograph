use codemap::{CodeMap, Span};
use core_model_builder::{
    ast::ast_types::{AstAnnotationParams, AstExpr},
    typechecker::Typed,
};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ResolvedAccess {
    pub value: AstExpr<Typed>,
}

impl ResolvedAccess {
    fn restrictive() -> Self {
        ResolvedAccess {
            value: AstExpr::BooleanLiteral(false, null_span()),
        }
    }
}

fn null_span() -> Span {
    let mut codemap = CodeMap::new();
    let file = codemap.add_file("".to_string(), "".to_string());
    file.span
}

pub fn build_access(
    access_annotation_params: Option<&AstAnnotationParams<Typed>>,
) -> ResolvedAccess {
    match access_annotation_params {
        Some(p) => {
            let value = match p {
                AstAnnotationParams::Single(default, _) => default,
                _ => panic!(), // service queries and annotations should only have a single parameter (the default value)
            };

            ResolvedAccess {
                value: value.clone(),
            }
        }
        None => ResolvedAccess::restrictive(),
    }
}
