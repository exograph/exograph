use std::collections::HashMap;

use anyhow::Result;
use payas_model::model::mapped_arena::MappedArena;
use serde::{Deserialize, Serialize};

use crate::ast::ast_types::{AstAnnotationParams, AstExpr, Untyped};

use super::{TypecheckFrom, Typed};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum TypedAnnotationParams {
    None,
    Single(AstExpr<Typed>),
    Map(HashMap<String, AstExpr<Typed>>),
}

impl TypecheckFrom<AstAnnotationParams<Untyped>> for TypedAnnotationParams {
    fn shallow(
        untyped: &AstAnnotationParams<Untyped>,
        errors: &mut Vec<codemap_diagnostic::Diagnostic>,
    ) -> Result<TypedAnnotationParams> {
        Ok(match untyped {
            AstAnnotationParams::None => TypedAnnotationParams::None,
            AstAnnotationParams::Single(expr, _) => {
                TypedAnnotationParams::Single(AstExpr::shallow(expr, errors)?)
            }
            AstAnnotationParams::Map(params, _) => TypedAnnotationParams::Map(
                params
                    .iter()
                    .map(|(name, expr)| AstExpr::shallow(expr, errors).map(|t| (name.clone(), t)))
                    .collect::<Result<_, _>>()?,
            ),
        })
    }

    fn pass(
        &mut self,
        env: &MappedArena<super::Type>,
        scope: &super::Scope,
        errors: &mut Vec<codemap_diagnostic::Diagnostic>,
    ) -> bool {
        match self {
            TypedAnnotationParams::None => true,
            TypedAnnotationParams::Single(expr_typ) => expr_typ.pass(env, scope, errors),
            TypedAnnotationParams::Map(params_typ) => params_typ
                .values_mut()
                .map(|param| param.pass(env, scope, errors))
                .any(|b| b),
        }
    }
}
