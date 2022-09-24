use codemap::{CodeMap, Span};
use payas_core_model_builder::{
    ast::ast_types::{AstAnnotationParams, AstExpr},
    typechecker::Typed,
};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ResolvedAccess {
    pub value: AstExpr<Typed>,
}

impl ResolvedAccess {
    fn permissive() -> Self {
        ResolvedAccess {
            value: AstExpr::BooleanLiteral(true, null_span()),
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
            let restrictive: AstExpr<Typed> = AstExpr::BooleanLiteral(false, null_span());

            let value = match p {
                AstAnnotationParams::Single(default, _) => default,

                _ => panic!(),
            };

            ResolvedAccess {
                value: value.clone(),
            }
        }
        None => ResolvedAccess::permissive(),
    }
}
