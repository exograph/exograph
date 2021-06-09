use payas_model::model::mapped_arena::MappedArena;
use serde::{Deserialize, Serialize};

use crate::ast::ast_types::RelationalOp;

use super::{PrimitiveType, Scope, Type, Typecheck, TypedExpression};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum TypedRelationalOp {
    Eq(Box<TypedExpression>, Box<TypedExpression>, Type),
    Neq(Box<TypedExpression>, Box<TypedExpression>, Type),
    Lt(Box<TypedExpression>, Box<TypedExpression>, Type),
    Lte(Box<TypedExpression>, Box<TypedExpression>, Type),
    Gt(Box<TypedExpression>, Box<TypedExpression>, Type),
    Gte(Box<TypedExpression>, Box<TypedExpression>, Type),
}

impl TypedRelationalOp {
    pub fn typ(&self) -> &Type {
        match &self {
            TypedRelationalOp::Eq(_, _, typ) => typ,
            TypedRelationalOp::Neq(_, _, typ) => typ,
            TypedRelationalOp::Lt(_, _, typ) => typ,
            TypedRelationalOp::Lte(_, _, typ) => typ,
            TypedRelationalOp::Gt(_, _, typ) => typ,
            TypedRelationalOp::Gte(_, _, typ) => typ,
        }
    }
}

impl Typecheck<TypedRelationalOp> for RelationalOp {
    fn shallow(&self) -> TypedRelationalOp {
        match &self {
            RelationalOp::Eq(left, right) => TypedRelationalOp::Eq(
                Box::new(left.shallow()),
                Box::new(right.shallow()),
                Type::Defer,
            ),
            RelationalOp::Neq(left, right) => TypedRelationalOp::Neq(
                Box::new(left.shallow()),
                Box::new(right.shallow()),
                Type::Defer,
            ),
            RelationalOp::Lt(left, right) => TypedRelationalOp::Lt(
                Box::new(left.shallow()),
                Box::new(right.shallow()),
                Type::Defer,
            ),
            RelationalOp::Lte(left, right) => TypedRelationalOp::Lte(
                Box::new(left.shallow()),
                Box::new(right.shallow()),
                Type::Defer,
            ),
            RelationalOp::Gt(left, right) => TypedRelationalOp::Gt(
                Box::new(left.shallow()),
                Box::new(right.shallow()),
                Type::Defer,
            ),
            RelationalOp::Gte(left, right) => TypedRelationalOp::Gte(
                Box::new(left.shallow()),
                Box::new(right.shallow()),
                Type::Defer,
            ),
        }
    }

    fn pass(&self, typ: &mut TypedRelationalOp, env: &MappedArena<Type>, scope: &Scope) -> bool {
        match &self {
            RelationalOp::Eq(left, right) => {
                if let TypedRelationalOp::Eq(left_typ, right_typ, o_typ) = typ {
                    let in_updated =
                        left.pass(left_typ, env, scope) || right.pass(right_typ, env, scope);
                    let out_updated = if o_typ.is_incomplete() {
                        if left_typ.typ().deref(env) == right_typ.typ().deref(env) {
                            *o_typ = Type::Primitive(PrimitiveType::Boolean);
                            true
                        } else {
                            *o_typ = Type::Error(format!(
                                "Mismatched types, comparing {:?} with {:?}",
                                left_typ.typ().deref(env),
                                right_typ.typ().deref(env)
                            ));
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
            RelationalOp::Neq(left, right) => {
                if let TypedRelationalOp::Neq(left_typ, right_typ, _) = typ {
                    let in_updated =
                        left.pass(left_typ, env, scope) || right.pass(right_typ, env, scope);
                    let out_updated = false;
                    in_updated || out_updated
                } else {
                    panic!()
                }
            }
            RelationalOp::Lt(left, right) => {
                if let TypedRelationalOp::Lt(left_typ, right_typ, _) = typ {
                    let in_updated =
                        left.pass(left_typ, env, scope) || right.pass(right_typ, env, scope);
                    let out_updated = false;
                    in_updated || out_updated
                } else {
                    panic!()
                }
            }
            RelationalOp::Lte(left, right) => {
                if let TypedRelationalOp::Lte(left_typ, right_typ, _) = typ {
                    let in_updated =
                        left.pass(left_typ, env, scope) || right.pass(right_typ, env, scope);
                    let out_updated = false;
                    in_updated || out_updated
                } else {
                    panic!()
                }
            }
            RelationalOp::Gt(left, right) => {
                if let TypedRelationalOp::Gt(left_typ, right_typ, _) = typ {
                    let in_updated =
                        left.pass(left_typ, env, scope) || right.pass(right_typ, env, scope);
                    let out_updated = false;
                    in_updated || out_updated
                } else {
                    panic!()
                }
            }
            RelationalOp::Gte(left, right) => {
                if let TypedRelationalOp::Gte(left_typ, right_typ, _) = typ {
                    let in_updated =
                        left.pass(left_typ, env, scope) || right.pass(right_typ, env, scope);
                    let out_updated = false;
                    in_updated || out_updated
                } else {
                    panic!()
                }
            }
        }
    }
}
