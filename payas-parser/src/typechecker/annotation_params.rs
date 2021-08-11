use std::collections::HashMap;

use codemap_diagnostic::Diagnostic;
use payas_model::model::mapped_arena::MappedArena;

use crate::ast::ast_types::{AstAnnotationParams, AstExpr, Untyped};

use super::annotation::AnnotationSpec;
use super::{Type, TypecheckFrom, Typed};

impl AstAnnotationParams<Typed> {
    pub fn as_single(&self) -> &AstExpr<Typed> {
        match self {
            Self::Single(expr, _) => expr,
            _ => panic!(),
        }
    }

    pub fn as_map(&self) -> &HashMap<String, AstExpr<Typed>> {
        match self {
            Self::Map(map, _) => map,
            _ => panic!(),
        }
    }
}

impl TypecheckFrom<AstAnnotationParams<Untyped>> for AstAnnotationParams<Typed> {
    fn shallow(untyped: &AstAnnotationParams<Untyped>) -> AstAnnotationParams<Typed> {
        match untyped {
            AstAnnotationParams::None => AstAnnotationParams::None,
            AstAnnotationParams::Single(expr, span) => {
                AstAnnotationParams::Single(AstExpr::shallow(expr), *span)
            }
            AstAnnotationParams::Map(params, spans) => AstAnnotationParams::Map(
                params
                    .iter()
                    .map(|(name, expr)| (name.clone(), AstExpr::shallow(expr)))
                    .collect(),
                spans.clone(),
            ),
        }
    }

    fn pass(
        &mut self,
        type_env: &MappedArena<Type>,
        annotation_env: &HashMap<String, AnnotationSpec>,
        scope: &super::Scope,
        errors: &mut Vec<Diagnostic>,
    ) -> bool {
        match self {
            AstAnnotationParams::None => false,
            AstAnnotationParams::Single(expr, _) => {
                expr.pass(type_env, annotation_env, scope, errors)
            }
            AstAnnotationParams::Map(params, _) => {
                params
                    .values_mut()
                    .map(|param| param.pass(type_env, annotation_env, scope, errors))
                    .filter(|b| *b)
                    .count()
                    > 0
            }
        }
    }
}
