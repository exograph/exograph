use std::collections::HashMap;

use codemap_diagnostic::{Diagnostic, Level, SpanLabel, SpanStyle};
use payas_model::model::mapped_arena::MappedArena;

use crate::ast::ast_types::{AstExpr, RelationalOp, Untyped};

use super::annotation::AnnotationSpec;
use super::{PrimitiveType, Scope, Type, TypecheckFrom, Typed};

impl RelationalOp<Typed> {
    pub fn typ(&self) -> &Type {
        match &self {
            RelationalOp::Eq(_, _, typ) => typ,
            RelationalOp::Neq(_, _, typ) => typ,
            RelationalOp::Lt(_, _, typ) => typ,
            RelationalOp::Lte(_, _, typ) => typ,
            RelationalOp::Gt(_, _, typ) => typ,
            RelationalOp::Gte(_, _, typ) => typ,
            RelationalOp::In(_, _, typ) => typ,
        }
    }
}

impl TypecheckFrom<RelationalOp<Untyped>> for RelationalOp<Typed> {
    fn shallow(untyped: &RelationalOp<Untyped>) -> RelationalOp<Typed> {
        match untyped {
            RelationalOp::Eq(left, right, _) => RelationalOp::Eq(
                Box::new(AstExpr::shallow(left)),
                Box::new(AstExpr::shallow(right)),
                Type::Defer,
            ),
            RelationalOp::Neq(left, right, _) => RelationalOp::Neq(
                Box::new(AstExpr::shallow(left)),
                Box::new(AstExpr::shallow(right)),
                Type::Defer,
            ),
            RelationalOp::Lt(left, right, _) => RelationalOp::Lt(
                Box::new(AstExpr::shallow(left)),
                Box::new(AstExpr::shallow(right)),
                Type::Defer,
            ),
            RelationalOp::Lte(left, right, _) => RelationalOp::Lte(
                Box::new(AstExpr::shallow(left)),
                Box::new(AstExpr::shallow(right)),
                Type::Defer,
            ),
            RelationalOp::Gt(left, right, _) => RelationalOp::Gt(
                Box::new(AstExpr::shallow(left)),
                Box::new(AstExpr::shallow(right)),
                Type::Defer,
            ),
            RelationalOp::Gte(left, right, _) => RelationalOp::Gte(
                Box::new(AstExpr::shallow(left)),
                Box::new(AstExpr::shallow(right)),
                Type::Defer,
            ),
            RelationalOp::In(left, right, _) => RelationalOp::In(
                Box::new(AstExpr::shallow(left)),
                Box::new(AstExpr::shallow(right)),
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
                    *o_typ = Type::Primitive(PrimitiveType::Boolean);
                    true
                } else {
                    *o_typ = Type::Error;

                    if left_typ.is_complete() && right_typ.is_complete() {
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
        };

        fn identical_match(left: &Type, right: &Type) -> bool {
            left == right
        }

        fn in_relation_match(left: &Type, right: &Type) -> bool {
            match right {
                Type::Array(inner) => *left == **inner,
                Type::Set(inner) => *left == **inner,
                _ => false,
            }
        }

        match self {
            RelationalOp::Eq(left, right, o_typ)
            | RelationalOp::Neq(left, right, o_typ)
            | RelationalOp::Lt(left, right, o_typ)
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
