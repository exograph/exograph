use anyhow::Result;
use codemap_diagnostic::{Diagnostic, Level, SpanLabel, SpanStyle};
use payas_model::model::mapped_arena::MappedArena;

use crate::ast::ast_types::{LogicalOp, Untyped};

use super::{PrimitiveType, Scope, Type, Typecheck, Typed};

impl LogicalOp<Typed> {
    pub fn typ(&self) -> &Type {
        match &self {
            LogicalOp::Not(_, _, typ) => typ,
            LogicalOp::And(_, _, typ) => typ,
            LogicalOp::Or(_, _, typ) => typ,
        }
    }
}
impl Typecheck<LogicalOp<Typed>> for LogicalOp<Untyped> {
    fn shallow(
        &self,
        errors: &mut Vec<codemap_diagnostic::Diagnostic>,
    ) -> Result<LogicalOp<Typed>> {
        Ok(match &self {
            LogicalOp::Not(v, s, _) => {
                LogicalOp::Not(Box::new(v.shallow(errors)?), *s, Type::Defer)
            }
            LogicalOp::And(left, right, _) => LogicalOp::And(
                Box::new(left.shallow(errors)?),
                Box::new(right.shallow(errors)?),
                Type::Defer,
            ),
            LogicalOp::Or(left, right, _) => LogicalOp::Or(
                Box::new(left.shallow(errors)?),
                Box::new(right.shallow(errors)?),
                Type::Defer,
            ),
        })
    }

    fn pass(
        &self,
        typ: &mut LogicalOp<Typed>,
        env: &MappedArena<Type>,
        scope: &Scope,
        errors: &mut Vec<codemap_diagnostic::Diagnostic>,
    ) -> bool {
        match &self {
            LogicalOp::Not(v, _, _) => {
                if let LogicalOp::Not(v_typ, _, o_typ) = typ {
                    let in_updated = v.pass(v_typ, env, scope, errors);
                    let out_updated = if o_typ.is_incomplete() {
                        match v_typ.typ().deref(env) {
                            Type::Primitive(PrimitiveType::Boolean) => {
                                *o_typ = Type::Primitive(PrimitiveType::Boolean);
                                true
                            }

                            other => {
                                *o_typ = Type::Error;
                                if !other.is_incomplete() {
                                    errors.push(Diagnostic {
                                        level: Level::Error,
                                        message: format!(
                                            "Cannot negate non-boolean type {}",
                                            &other
                                        ),
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
                } else {
                    panic!()
                }
            }
            LogicalOp::And(left, right, _) => {
                if let LogicalOp::And(left_typ, right_typ, o_typ) = typ {
                    let in_updated = left.pass(left_typ, env, scope, errors)
                        || right.pass(right_typ, env, scope, errors);
                    let out_updated = if o_typ.is_incomplete() {
                        let left_typ = left_typ.typ().deref(env);
                        let right_typ = right_typ.typ().deref(env);
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
                } else {
                    panic!()
                }
            }
            LogicalOp::Or(left, right, _) => {
                if let LogicalOp::Or(left_typ, right_typ, o_typ) = typ {
                    let in_updated = left.pass(left_typ, env, scope, errors)
                        || right.pass(right_typ, env, scope, errors);
                    let out_updated = if o_typ.is_incomplete() {
                        let left_typ = left_typ.typ().deref(env);
                        let right_typ = right_typ.typ().deref(env);

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
                } else {
                    panic!()
                }
            }
        }
    }
}
