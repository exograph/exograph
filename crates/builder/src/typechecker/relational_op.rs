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

use crate::ast::ast_types::{AstExpr, RelationalOp, Untyped};

use super::{PrimitiveType, Scope, Type, TypecheckFrom};

impl TypecheckFrom<RelationalOp<Untyped>> for RelationalOp<Typed> {
    fn shallow(untyped: &RelationalOp<Untyped>) -> RelationalOp<Typed> {
        let (left, right) = untyped.sides();

        let combiner = match untyped {
            RelationalOp::Eq(..) => RelationalOp::Eq,
            RelationalOp::Neq(..) => RelationalOp::Neq,
            RelationalOp::Lt(..) => RelationalOp::Lt,
            RelationalOp::Lte(..) => RelationalOp::Lte,
            RelationalOp::Gt(..) => RelationalOp::Gt,
            RelationalOp::Gte(..) => RelationalOp::Gte,
            RelationalOp::In(..) => RelationalOp::In,
        };

        let left = Box::new(AstExpr::shallow(left));
        let right = Box::new(AstExpr::shallow(right));

        combiner(left, right, Type::Defer)
    }

    fn pass(
        &mut self,
        type_env: &MappedArena<Type>,
        annotation_env: &HashMap<String, AnnotationSpec>,
        scope: &Scope,
        errors: &mut Vec<Diagnostic>,
    ) -> bool {
        let mut typecheck_operands = |left: &mut Box<AstExpr<Typed>>,
                                      right: &mut Box<AstExpr<Typed>>,
                                      o_typ: &mut Type,
                                      type_match: fn(&Type, &Type) -> bool|
         -> bool {
            let in_updated = left.pass(type_env, annotation_env, scope, errors)
                || right.pass(type_env, annotation_env, scope, errors);
            let out_updated = if o_typ.is_incomplete() {
                let left_typ = left.typ().deref(type_env);
                let right_typ = right.typ().deref(type_env);

                if left_typ.is_complete()
                    && right_typ.is_complete()
                    && type_match(&left_typ, &right_typ)
                {
                    *o_typ = Type::Primitive(PrimitiveType::Plain(primitive_type::BOOLEAN_TYPE));
                    true
                } else {
                    *o_typ = Type::Error;

                    if left_typ.is_complete() && right_typ.is_complete() {
                        let mut spans = vec![];
                        spans.push(SpanLabel {
                            span: left.span(),
                            style: SpanStyle::Primary,
                            label: Some(format!("got {left_typ}")),
                        });

                        spans.push(SpanLabel {
                            span: right.span(),
                            style: SpanStyle::Primary,
                            label: Some(format!("got {right_typ}")),
                        });

                        errors.push(Diagnostic {
                            level: Level::Error,
                            message: format!(
                                "Mismatched types, comparing {left_typ} with {right_typ}"
                            ),
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
        };

        fn in_relation_match(left: &Type, right: &Type) -> bool {
            match right {
                Type::Array(inner) => *left == **inner,
                Type::Set(inner) => *left == **inner,
                _ => false,
            }
        }

        match self {
            RelationalOp::Eq(left, right, o_typ) | RelationalOp::Neq(left, right, o_typ) => {
                // For equality and inequality, we allow optional types to be compared to null
                typecheck_operands(left, right, o_typ, identical_match_with_null)
            }
            RelationalOp::Lt(left, right, o_typ)
            | RelationalOp::Lte(left, right, o_typ)
            | RelationalOp::Gt(left, right, o_typ)
            | RelationalOp::Gte(left, right, o_typ) => {
                typecheck_operands(left, right, o_typ, identical_match)
            }
            RelationalOp::In(left, right, o_typ) => {
                typecheck_operands(left, right, o_typ, in_relation_match)
            }
        }
    }
}

fn identical_match_with_null(left: &Type, right: &Type) -> bool {
    // Allow optional types to be compared to null
    match (left, right) {
        (Type::Optional(_), Type::Null)
        | (Type::Null, Type::Optional(_))
        | (Type::Null, Type::Null) => true,
        _ => identical_match(left, right),
    }
}

pub fn identical_match(left: &Type, right: &Type) -> bool {
    fn matches(left: &Type, right: &Type) -> bool {
        match (left, right) {
            (Type::Primitive(left_typ), Type::Primitive(right_typ)) => {
                match (left_typ, right_typ) {
                    (
                        PrimitiveType::Plain(left_primitive),
                        PrimitiveType::Plain(right_primitive),
                    ) if (left_primitive.name() == primitive_type::FloatType::NAME
                        && right_primitive.name() == primitive_type::IntType::NAME)
                        || (left_primitive.name() == primitive_type::IntType::NAME
                            && right_primitive.name() == primitive_type::FloatType::NAME) =>
                    {
                        true
                    }
                    _ => left_typ == right_typ,
                }
            }
            _ => left == right,
        }
    }

    // Allow optional types to be compared to non-optional types
    match (left, right) {
        (Type::Optional(left), Type::Optional(right)) => matches(left, right),
        (Type::Optional(left), right) => matches(left, right),
        (left, Type::Optional(right)) => matches(left, right),
        _ => matches(left, right),
    }
}
