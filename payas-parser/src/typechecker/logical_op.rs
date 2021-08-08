use anyhow::Result;
use codemap_diagnostic::{Diagnostic, Level, SpanLabel, SpanStyle};
use payas_model::model::mapped_arena::MappedArena;

use crate::ast::ast_types::{AstExpr, LogicalOp, Untyped};

use super::{PrimitiveType, Scope, Type, TypecheckNew, Typed};

impl LogicalOp<Typed> {
    pub fn typ(&self) -> &Type {
        match &self {
            LogicalOp::Not(_, _, typ) => typ,
            LogicalOp::And(_, _, typ) => typ,
            LogicalOp::Or(_, _, typ) => typ,
        }
    }
}
impl TypecheckNew<LogicalOp<Untyped>> for LogicalOp<Typed> {
    fn shallow(
        untyped: &LogicalOp<Untyped>,
        errors: &mut Vec<codemap_diagnostic::Diagnostic>,
    ) -> Result<LogicalOp<Typed>> {
        Ok(match untyped {
            LogicalOp::Not(v, s, _) => {
                LogicalOp::Not(Box::new(AstExpr::shallow(v, errors)?), *s, Type::Defer)
            }
            LogicalOp::And(left, right, _) => LogicalOp::And(
                Box::new(AstExpr::shallow(left, errors)?),
                Box::new(AstExpr::shallow(right, errors)?),
                Type::Defer,
            ),
            LogicalOp::Or(left, right, _) => LogicalOp::Or(
                Box::new(AstExpr::shallow(left, errors)?),
                Box::new(AstExpr::shallow(right, errors)?),
                Type::Defer,
            ),
        })
    }

    fn pass(
        &mut self,
        env: &MappedArena<Type>,
        scope: &Scope,
        errors: &mut Vec<codemap_diagnostic::Diagnostic>,
    ) -> bool {
        match self {
            LogicalOp::Not(v, _, o_typ) => {
                let in_updated = v.pass(env, scope, errors);
                let out_updated = if o_typ.is_incomplete() {
                    match v.typ().deref(env) {
                        Type::Primitive(PrimitiveType::Boolean) => {
                            *o_typ = Type::Primitive(PrimitiveType::Boolean);
                            true
                        }

                        other => {
                            *o_typ = Type::Error;
                            if !other.is_incomplete() {
                                errors.push(Diagnostic {
                                    level: Level::Error,
                                    message: format!("Cannot negate non-boolean type {}", &other),
                                    code: Some("C000".to_string()),
                                    spans: vec![SpanLabel {
                                        span: *v.span(),
                                        style: SpanStyle::Primary,
                                        label: Some(format!("expected Boolean, got {}", other)),
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
            LogicalOp::And(left, right, o_typ) => {
                let in_updated = left.pass(env, scope, errors) || right.pass(env, scope, errors);
                let out_updated = if o_typ.is_incomplete() {
                    let left_typ = left.typ().deref(env);
                    let right_typ = right.typ().deref(env);
                    if left_typ == Type::Primitive(PrimitiveType::Boolean)
                        && right_typ == Type::Primitive(PrimitiveType::Boolean)
                    {
                        *o_typ = Type::Primitive(PrimitiveType::Boolean);
                        true
                    } else {
                        *o_typ = Type::Error;

                        if !left_typ.is_incomplete() || !right_typ.is_incomplete() {
                            let mut spans = vec![];
                            if left_typ != Type::Primitive(PrimitiveType::Boolean)
                                && !left_typ.is_incomplete()
                            {
                                spans.push(SpanLabel {
                                    span: *left.span(),
                                    style: SpanStyle::Primary,
                                    label: Some(format!("expected Boolean, got {}", left_typ)),
                                })
                            }

                            if right_typ != Type::Primitive(PrimitiveType::Boolean)
                                && !left_typ.is_incomplete()
                            {
                                spans.push(SpanLabel {
                                    span: *right.span(),
                                    style: SpanStyle::Primary,
                                    label: Some(format!("expected Boolean, got {}", right_typ)),
                                })
                            }

                            errors.push(Diagnostic {
                                level: Level::Error,
                                message: "Both inputs to && must be booleans".to_string(),
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
            LogicalOp::Or(left, right, o_typ) => {
                let in_updated = left.pass(env, scope, errors) || right.pass(env, scope, errors);
                let out_updated = if o_typ.is_incomplete() {
                    let left_typ = left.typ().deref(env);
                    let right_typ = right.typ().deref(env);

                    if left_typ == Type::Primitive(PrimitiveType::Boolean)
                        && right_typ == Type::Primitive(PrimitiveType::Boolean)
                    {
                        *o_typ = Type::Primitive(PrimitiveType::Boolean);
                        true
                    } else {
                        *o_typ = Type::Error;

                        if !left_typ.is_incomplete() || !right_typ.is_incomplete() {
                            let mut spans = vec![];
                            if left_typ != Type::Primitive(PrimitiveType::Boolean)
                                && !left_typ.is_incomplete()
                            {
                                spans.push(SpanLabel {
                                    span: *left.span(),
                                    style: SpanStyle::Primary,
                                    label: Some(format!("expected Boolean, got {}", left_typ)),
                                })
                            }

                            if right_typ != Type::Primitive(PrimitiveType::Boolean)
                                && !right_typ.is_incomplete()
                            {
                                spans.push(SpanLabel {
                                    span: *right.span(),
                                    style: SpanStyle::Primary,
                                    label: Some(format!("expected Boolean, got {}", right_typ)),
                                })
                            }

                            errors.push(Diagnostic {
                                level: Level::Error,
                                message: "Both inputs to || must be booleans".to_string(),
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
