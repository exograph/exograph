use anyhow::Result;
use codemap_diagnostic::{Diagnostic, Level, SpanLabel, SpanStyle};
use payas_model::model::mapped_arena::MappedArena;

use crate::ast::ast_types::{RelationalOp, Untyped};

use super::{PrimitiveType, Scope, Type, Typecheck, Typed};

impl RelationalOp<Typed> {
    pub fn typ(&self) -> &Type {
        match &self {
            RelationalOp::Eq(_, _, typ) => typ,
            RelationalOp::Neq(_, _, typ) => typ,
            RelationalOp::Lt(_, _, typ) => typ,
            RelationalOp::Lte(_, _, typ) => typ,
            RelationalOp::Gt(_, _, typ) => typ,
            RelationalOp::Gte(_, _, typ) => typ,
        }
    }
}

impl Typecheck<RelationalOp<Typed>> for RelationalOp<Untyped> {
    fn shallow(
        &self,
        errors: &mut Vec<codemap_diagnostic::Diagnostic>,
    ) -> Result<RelationalOp<Typed>> {
        Ok(match &self {
            RelationalOp::Eq(left, right, _) => RelationalOp::Eq(
                Box::new(left.shallow(errors)?),
                Box::new(right.shallow(errors)?),
                Type::Defer,
            ),
            RelationalOp::Neq(left, right, _) => RelationalOp::Neq(
                Box::new(left.shallow(errors)?),
                Box::new(right.shallow(errors)?),
                Type::Defer,
            ),
            RelationalOp::Lt(left, right, _) => RelationalOp::Lt(
                Box::new(left.shallow(errors)?),
                Box::new(right.shallow(errors)?),
                Type::Defer,
            ),
            RelationalOp::Lte(left, right, _) => RelationalOp::Lte(
                Box::new(left.shallow(errors)?),
                Box::new(right.shallow(errors)?),
                Type::Defer,
            ),
            RelationalOp::Gt(left, right, _) => RelationalOp::Gt(
                Box::new(left.shallow(errors)?),
                Box::new(right.shallow(errors)?),
                Type::Defer,
            ),
            RelationalOp::Gte(left, right, _) => RelationalOp::Gte(
                Box::new(left.shallow(errors)?),
                Box::new(right.shallow(errors)?),
                Type::Defer,
            ),
        })
    }

    fn pass(
        &self,
        typ: &mut RelationalOp<Typed>,
        env: &MappedArena<Type>,
        scope: &Scope,
        errors: &mut Vec<codemap_diagnostic::Diagnostic>,
    ) -> bool {
        match &self {
            RelationalOp::Eq(left, right, _) => {
                if let RelationalOp::Eq(left_typ, right_typ, o_typ) = typ {
                    let in_updated = left.pass(left_typ, env, scope, errors)
                        || right.pass(right_typ, env, scope, errors);
                    let out_updated = if o_typ.is_incomplete() {
                        let left_typ = left_typ.typ().deref(env);
                        let right_typ = right_typ.typ().deref(env);
                        if left_typ == right_typ && !left_typ.is_incomplete() {
                            *o_typ = Type::Primitive(PrimitiveType::Boolean);
                            true
                        } else {
                            *o_typ = Type::Error;

                            if !left_typ.is_incomplete() && !right_typ.is_incomplete() {
                                let mut spans = vec![];
                                spans.push(SpanLabel {
                                    span: *left.span(),
                                    style: SpanStyle::Primary,
                                    label: Some(format!("got {}", left_typ)),
                                });

                                spans.push(SpanLabel {
                                    span: *right.span(),
                                    style: SpanStyle::Primary,
                                    label: Some(format!("got {}", right_typ)),
                                });

                                errors.push(Diagnostic {
                                    level: Level::Error,
                                    message: format!(
                                        "Mismatched types, comparing {} with {}",
                                        left_typ, right_typ
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
                } else {
                    panic!()
                }
            }
            RelationalOp::Neq(left, right, _) => {
                if let RelationalOp::Neq(left_typ, right_typ, _) = typ {
                    let in_updated = left.pass(left_typ, env, scope, errors)
                        || right.pass(right_typ, env, scope, errors);
                    let out_updated = false;
                    in_updated || out_updated
                } else {
                    panic!()
                }
            }
            RelationalOp::Lt(left, right, _) => {
                if let RelationalOp::Lt(left_typ, right_typ, _) = typ {
                    let in_updated = left.pass(left_typ, env, scope, errors)
                        || right.pass(right_typ, env, scope, errors);
                    let out_updated = false;
                    in_updated || out_updated
                } else {
                    panic!()
                }
            }
            RelationalOp::Lte(left, right, _) => {
                if let RelationalOp::Lte(left_typ, right_typ, _) = typ {
                    let in_updated = left.pass(left_typ, env, scope, errors)
                        || right.pass(right_typ, env, scope, errors);
                    let out_updated = false;
                    in_updated || out_updated
                } else {
                    panic!()
                }
            }
            RelationalOp::Gt(left, right, _) => {
                if let RelationalOp::Gt(left_typ, right_typ, _) = typ {
                    let in_updated = left.pass(left_typ, env, scope, errors)
                        || right.pass(right_typ, env, scope, errors);
                    let out_updated = false;
                    in_updated || out_updated
                } else {
                    panic!()
                }
            }
            RelationalOp::Gte(left, right, _) => {
                if let RelationalOp::Gte(left_typ, right_typ, _) = typ {
                    let in_updated = left.pass(left_typ, env, scope, errors)
                        || right.pass(right_typ, env, scope, errors);
                    let out_updated = false;
                    in_updated || out_updated
                } else {
                    panic!()
                }
            }
        }
    }
}
