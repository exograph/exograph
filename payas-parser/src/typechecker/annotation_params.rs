use std::collections::HashMap;

use codemap_diagnostic::Diagnostic;
use payas_core_model_builder::typechecker::{annotation::AnnotationSpec, Typed};
use payas_model::model::mapped_arena::MappedArena;

use crate::ast::ast_types::{AstAnnotationParams, AstExpr, Untyped};

use super::{Type, TypecheckFrom};

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
