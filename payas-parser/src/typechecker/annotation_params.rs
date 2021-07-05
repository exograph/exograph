use std::collections::HashMap;

use payas_model::model::mapped_arena::MappedArena;
use serde::{Deserialize, Serialize};

use crate::ast::ast_types::AstAnnotationParams;

use super::{Typecheck, TypedExpression};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum TypedAnnotationParams {
    None,
    Single(TypedExpression),
    Map(HashMap<String, TypedExpression>),
}

impl Typecheck<TypedAnnotationParams> for AstAnnotationParams {
    fn shallow(&self) -> TypedAnnotationParams {
        match &self {
            AstAnnotationParams::None => TypedAnnotationParams::None,
            AstAnnotationParams::Single(expr) => TypedAnnotationParams::Single(expr.shallow()),
            AstAnnotationParams::Map(params) => TypedAnnotationParams::Map(
                params
                    .clone()
                    .into_iter()
                    .map(|(name, expr)| (name, expr.shallow()))
                    .collect(),
            ),
        }
    }

    fn pass(
        &self,
        typ: &mut TypedAnnotationParams,
        env: &MappedArena<super::Type>,
        scope: &super::Scope,
        errors: &mut Vec<codemap_diagnostic::Diagnostic>,
    ) -> bool {
        match &self {
            AstAnnotationParams::None => true,
            AstAnnotationParams::Single(expr) => {
                if let TypedAnnotationParams::Single(expr_typ) = typ {
                    expr.pass(expr_typ, env, scope, errors)
                } else {
                    panic!();
                }
            }
            AstAnnotationParams::Map(params) => {
                if let TypedAnnotationParams::Map(params_typ) = typ {
                    params.iter().any(|(name, expr)| {
                        expr.pass(params_typ.get_mut(name).unwrap(), env, scope, errors)
                    })
                } else {
                    panic!();
                }
            }
        }
    }
}
