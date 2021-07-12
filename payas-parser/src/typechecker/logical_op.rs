use anyhow::Result;
use codemap_diagnostic::{Diagnostic, Level, SpanLabel, SpanStyle};
use payas_model::model::mapped_arena::MappedArena;

use crate::ast::ast_types::LogicalOp;
use serde::{Deserialize, Serialize};

use super::{expression::TypedExpression, PrimitiveType, Scope, Type, Typecheck};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum TypedLogicalOp {
    Not(Box<TypedExpression>, Type),
    And(Box<TypedExpression>, Box<TypedExpression>, Type),
    Or(Box<TypedExpression>, Box<TypedExpression>, Type),
}

impl TypedLogicalOp {
    pub fn typ(&self) -> &Type {
        match &self {
            TypedLogicalOp::Not(_, typ) => typ,
            TypedLogicalOp::And(_, _, typ) => typ,
            TypedLogicalOp::Or(_, _, typ) => typ,
        }
    }
}
impl Typecheck<TypedLogicalOp> for LogicalOp {
    fn shallow(&self, errors: &mut Vec<codemap_diagnostic::Diagnostic>) -> Result<TypedLogicalOp> {
        Ok(match &self {
            LogicalOp::Not(v, _) => TypedLogicalOp::Not(Box::new(v.shallow(errors)?), Type::Defer),
            LogicalOp::And(left, right) => TypedLogicalOp::And(
                Box::new(left.shallow(errors)?),
                Box::new(right.shallow(errors)?),
                Type::Defer,
            ),
            LogicalOp::Or(left, right) => TypedLogicalOp::Or(
                Box::new(left.shallow(errors)?),
                Box::new(right.shallow(errors)?),
                Type::Defer,
            ),
        })
    }

    fn pass(
        &self,
        typ: &mut TypedLogicalOp,
        env: &MappedArena<Type>,
        scope: &Scope,
        errors: &mut Vec<codemap_diagnostic::Diagnostic>,
    ) -> bool {
        match &self {
            LogicalOp::Not(v, _) => {
                if let TypedLogicalOp::Not(v_typ, o_typ) = typ {
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
            LogicalOp::And(left, right) => {
                if let TypedLogicalOp::And(left_typ, right_typ, o_typ) = typ {
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
            LogicalOp::Or(left, right) => {
                if let TypedLogicalOp::Or(left_typ, right_typ, o_typ) = typ {
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
