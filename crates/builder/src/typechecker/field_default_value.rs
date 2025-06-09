// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::collections::HashMap;

use codemap_diagnostic::{Diagnostic, Level, SpanLabel, SpanStyle};
use core_model::mapped_arena::MappedArena;
use core_model_builder::typechecker::Typed;
use core_model_builder::typechecker::annotation::AnnotationSpec;

use crate::ast::ast_types::{AstExpr, AstFieldDefault, AstFieldDefaultKind, Untyped};

use super::{Scope, Type, TypecheckFrom};

impl TypecheckFrom<AstFieldDefault<Untyped>> for AstFieldDefault<Typed> {
    fn shallow(untyped: &AstFieldDefault<Untyped>) -> AstFieldDefault<Typed> {
        let kind = {
            match &untyped.kind {
                AstFieldDefaultKind::Function(fn_name, args) => AstFieldDefaultKind::Function(
                    fn_name.clone(),
                    args.iter().map(AstExpr::shallow).collect(),
                ),
                AstFieldDefaultKind::Value(expr) => {
                    AstFieldDefaultKind::Value(AstExpr::shallow(expr))
                }
            }
        };

        AstFieldDefault {
            kind,
            span: untyped.span,
        }
    }

    fn pass(
        &mut self,
        type_env: &MappedArena<Type>,
        annotation_env: &HashMap<String, AnnotationSpec>,
        scope: &Scope,
        errors: &mut Vec<Diagnostic>,
    ) -> bool {
        let mut check_literal = |expr: &mut AstExpr<Typed>| match expr {
            AstExpr::BooleanLiteral(_, _)
            | AstExpr::StringLiteral(_, _)
            | AstExpr::NumberLiteral(_, _)
            | AstExpr::FieldSelection(_) => expr.pass(type_env, annotation_env, scope, errors),

            _ => {
                errors.push(Diagnostic {
                    level: Level::Error,
                    message: "Must be a literal or a context field.".to_string(),
                    code: Some("C000".to_string()),
                    spans: vec![SpanLabel {
                        span: self.span,
                        style: SpanStyle::Primary,
                        label: Some("not a literal".to_string()),
                    }],
                });
                false
            }
        };

        match &mut self.kind {
            AstFieldDefaultKind::Function(_, args) => args.iter_mut().any(check_literal),
            AstFieldDefaultKind::Value(expr) => check_literal(expr),
        }
    }
}
