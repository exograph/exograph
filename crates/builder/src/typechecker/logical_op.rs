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
use core_model::{mapped_arena::MappedArena, primitive_type};
use core_model_builder::typechecker::{Typed, annotation::AnnotationSpec};

use crate::ast::ast_types::{AstExpr, LogicalOp, Untyped};

use super::{PrimitiveType, Scope, Type, TypecheckFrom};

impl TypecheckFrom<LogicalOp<Untyped>> for LogicalOp<Typed> {
    fn shallow(untyped: &LogicalOp<Untyped>) -> LogicalOp<Typed> {
        match untyped {
            LogicalOp::Not(v, s, _) => {
                LogicalOp::Not(Box::new(AstExpr::shallow(v)), *s, Type::Defer)
            }
            LogicalOp::And(left, right, s, _) => LogicalOp::And(
                Box::new(AstExpr::shallow(left)),
                Box::new(AstExpr::shallow(right)),
                *s,
                Type::Defer,
            ),
            LogicalOp::Or(left, right, s, _) => LogicalOp::Or(
                Box::new(AstExpr::shallow(left)),
                Box::new(AstExpr::shallow(right)),
                *s,
                Type::Defer,
            ),
        }
    }

    fn pass(
        &mut self,
        type_env: &MappedArena<Type>,
        annotation_env: &HashMap<String, AnnotationSpec>,
        scope: &Scope,
        errors: &mut Vec<Diagnostic>,
    ) -> bool {
        match self {
            LogicalOp::Not(v, _, o_typ) => {
                let in_updated = v.pass(type_env, annotation_env, scope, errors);
                let out_updated = if o_typ.is_incomplete() {
                    match v.typ().deref(type_env) {
                        Type::Primitive(PrimitiveType::Plain(primitive_type))
                            if primitive_type.name() == primitive_type::BooleanType::NAME =>
                        {
                            *o_typ =
                                Type::Primitive(PrimitiveType::Plain(primitive_type::BOOLEAN_TYPE));
                            true
                        }

                        other => {
                            *o_typ = Type::Error;
                            if other.is_complete() {
                                errors.push(Diagnostic {
                                    level: Level::Error,
                                    message: format!("Cannot negate non-boolean type {}", &other),
                                    code: Some("C000".to_string()),
                                    spans: vec![SpanLabel {
                                        span: v.span(),
                                        style: SpanStyle::Primary,
                                        label: Some(format!("expected Boolean, got {other}")),
                                    }],
                                });
                            }

                            false
                        }
                    }
                } else {
                    false
                };
                in_updated || out_updated
            }
            LogicalOp::Or(left, right, _, o_typ) | LogicalOp::And(left, right, _, o_typ) => {
                let in_updated = left.pass(type_env, annotation_env, scope, errors)
                    || right.pass(type_env, annotation_env, scope, errors);
                let out_updated = if o_typ.is_incomplete() {
                    let left_typ = left.typ().deref(type_env);
                    let right_typ = right.typ().deref(type_env);

                    if left_typ
                        == Type::Primitive(PrimitiveType::Plain(primitive_type::BOOLEAN_TYPE))
                        && right_typ
                            == Type::Primitive(PrimitiveType::Plain(primitive_type::BOOLEAN_TYPE))
                    {
                        *o_typ =
                            Type::Primitive(PrimitiveType::Plain(primitive_type::BOOLEAN_TYPE));
                        true
                    } else {
                        *o_typ = Type::Error;

                        if left_typ.is_complete() || right_typ.is_complete() {
                            let mut spans = vec![];
                            if left_typ
                                != Type::Primitive(PrimitiveType::Plain(
                                    primitive_type::BOOLEAN_TYPE,
                                ))
                                && left_typ.is_complete()
                            {
                                spans.push(SpanLabel {
                                    span: left.span(),
                                    style: SpanStyle::Primary,
                                    label: Some(format!("expected Boolean, got {left_typ}")),
                                })
                            }

                            if right_typ
                                != Type::Primitive(PrimitiveType::Plain(
                                    primitive_type::BOOLEAN_TYPE,
                                ))
                                && right_typ.is_complete()
                            {
                                spans.push(SpanLabel {
                                    span: right.span(),
                                    style: SpanStyle::Primary,
                                    label: Some(format!("expected Boolean, got {right_typ}")),
                                })
                            }

                            errors.push(Diagnostic {
                                level: Level::Error,
                                message: "Both inputs to a logical operation must be booleans"
                                    .to_string(),
                                code: Some("C000".to_string()),
                                spans,
                            });
                        }

                        false
                    }
                } else {
                    false
                };
                in_updated || out_updated
            }
        }
    }
}
