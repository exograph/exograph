use std::collections::HashMap;

use anyhow::Result;
use payas_model::model::mapped_arena::MappedArena;
use serde::{Deserialize, Serialize};

use crate::ast::ast_types::{AstAnnotationParams, AstExpr, Untyped};

use super::{Typecheck, Typed};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum TypedAnnotationParams {
    None,
    Single(AstExpr<Typed>),
    Map(HashMap<String, AstExpr<Typed>>),
}

impl Typecheck<TypedAnnotationParams> for AstAnnotationParams<Untyped> {
    fn shallow(
        &self,
        errors: &mut Vec<codemap_diagnostic::Diagnostic>,
    ) -> Result<TypedAnnotationParams> {
        Ok(match &self {
            AstAnnotationParams::None => TypedAnnotationParams::None,
            AstAnnotationParams::Single(expr, _) => {
                TypedAnnotationParams::Single(expr.shallow(errors)?)
            }
            AstAnnotationParams::Map(params, _) => TypedAnnotationParams::Map(
                params
                    .iter()
                    .map(|(name, expr)| expr.shallow(errors).map(|t| (name.clone(), t)))
                    .collect::<Result<_, _>>()?,
            ),
        })
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
            AstAnnotationParams::Single(expr, _) => {
                if let TypedAnnotationParams::Single(expr_typ) = typ {
                    expr.pass(expr_typ, env, scope, errors)
                } else {
                    panic!();
                }
            }
            AstAnnotationParams::Map(params, _) => {
                if let TypedAnnotationParams::Map(params_typ) = typ {
                    params
                        .iter()
                        .map(|(name, expr)| {
                            expr.pass(params_typ.get_mut(name).unwrap(), env, scope, errors)
                        })
                        .any(|b| b)
                } else {
                    panic!();
                }
            }
        }
    }
}
