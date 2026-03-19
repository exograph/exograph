// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::collections::HashMap;

use codemap_diagnostic::Diagnostic;
use core_model::mapped_arena::MappedArena;
use core_model_builder::typechecker::{Typed, annotation::AnnotationSpec};

use crate::ast::ast_types::{AstAccessExpr, AstAnnotationParam, AstAnnotationParams, Untyped};

use super::{Type, TypecheckFrom};

impl TypecheckFrom<AstAnnotationParam<Untyped>> for AstAnnotationParam<Typed> {
    fn shallow(untyped: &AstAnnotationParam<Untyped>) -> AstAnnotationParam<Typed> {
        match untyped {
            AstAnnotationParam::Literal(lit) => AstAnnotationParam::Literal(lit.clone()),
            AstAnnotationParam::StringList(v, s) => {
                AstAnnotationParam::StringList(v.clone(), s.clone())
            }
            AstAnnotationParam::ObjectLiteral(m, s) => {
                AstAnnotationParam::ObjectLiteral(m.clone(), *s)
            }
            AstAnnotationParam::AccessExpr(expr) => {
                AstAnnotationParam::AccessExpr(AstAccessExpr::shallow(expr))
            }
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
            AstAnnotationParam::Literal(_)
            | AstAnnotationParam::StringList(_, _)
            | AstAnnotationParam::ObjectLiteral(_, _) => false,
            AstAnnotationParam::AccessExpr(expr) => {
                expr.pass(type_env, annotation_env, scope, errors)
            }
        }
    }
}

impl TypecheckFrom<AstAnnotationParams<Untyped>> for AstAnnotationParams<Typed> {
    fn shallow(untyped: &AstAnnotationParams<Untyped>) -> AstAnnotationParams<Typed> {
        match untyped {
            AstAnnotationParams::None => AstAnnotationParams::None,
            AstAnnotationParams::Single(param, span) => {
                AstAnnotationParams::Single(AstAnnotationParam::shallow(param), *span)
            }
            AstAnnotationParams::Map(params, spans) => AstAnnotationParams::Map(
                params
                    .iter()
                    .map(|(name, param)| (name.clone(), AstAnnotationParam::shallow(param)))
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
            AstAnnotationParams::Single(param, _) => {
                param.pass(type_env, annotation_env, scope, errors)
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
