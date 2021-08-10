use std::collections::HashMap;

use payas_model::model::mapped_arena::MappedArena;
use serde::{Deserialize, Serialize};

use crate::ast::ast_types::AstAnnotationParams;

use super::annotation::AnnotationSpec;
use super::{Type, Typecheck, TypedExpression};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[allow(clippy::large_enum_variant)]
pub enum TypedAnnotationParams {
    None,
    Single(TypedExpression),
    Map(HashMap<String, TypedExpression>),
}

impl TypedAnnotationParams {
    pub fn as_single(&self) -> &TypedExpression {
        match self {
            Self::Single(expr) => expr,
            _ => panic!(),
        }
    }

    pub fn as_map(&self) -> &HashMap<String, TypedExpression> {
        match self {
            Self::Map(map) => map,
            _ => panic!(),
        }
    }
}

impl Typecheck<TypedAnnotationParams> for AstAnnotationParams {
    fn shallow(&self) -> TypedAnnotationParams {
        match &self {
            AstAnnotationParams::None => TypedAnnotationParams::None,
            AstAnnotationParams::Single(expr, _) => TypedAnnotationParams::Single(expr.shallow()),
            AstAnnotationParams::Map(params, _) => TypedAnnotationParams::Map(
                params
                    .iter()
                    .map(|(name, expr)| (name.clone(), expr.shallow()))
                    .collect(),
            ),
        }
    }

    fn pass(
        &self,
        typ: &mut TypedAnnotationParams,
        type_env: &MappedArena<Type>,
        annotation_env: &HashMap<String, AnnotationSpec>,
        scope: &super::Scope,
        errors: &mut Vec<codemap_diagnostic::Diagnostic>,
    ) -> bool {
        match &self {
            AstAnnotationParams::None => false,
            AstAnnotationParams::Single(expr, _) => {
                if let TypedAnnotationParams::Single(expr_typ) = typ {
                    expr.pass(expr_typ, type_env, annotation_env, scope, errors)
                } else {
                    panic!();
                }
            }
            AstAnnotationParams::Map(params, _) => {
                if let TypedAnnotationParams::Map(params_typ) = typ {
                    params
                        .iter()
                        .map(|(name, expr)| {
                            expr.pass(
                                params_typ.get_mut(name).unwrap(),
                                type_env,
                                annotation_env,
                                scope,
                                errors,
                            )
                        })
                        .any(|b| b)
                } else {
                    panic!();
                }
            }
        }
    }
}
