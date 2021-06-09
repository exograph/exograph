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
    fn shallow(&self) -> TypedLogicalOp {
        match &self {
            LogicalOp::Not(v) => TypedLogicalOp::Not(Box::new(v.shallow()), Type::Defer),
            LogicalOp::And(left, right) => TypedLogicalOp::And(
                Box::new(left.shallow()),
                Box::new(right.shallow()),
                Type::Defer,
            ),
            LogicalOp::Or(left, right) => TypedLogicalOp::Or(
                Box::new(left.shallow()),
                Box::new(right.shallow()),
                Type::Defer,
            ),
        }
    }

    fn pass(&self, typ: &mut TypedLogicalOp, env: &MappedArena<Type>, scope: &Scope) -> bool {
        match &self {
            LogicalOp::Not(v) => {
                if let TypedLogicalOp::Not(v_typ, o_typ) = typ {
                    let in_updated = v.pass(v_typ, env, scope);
                    let out_updated = if o_typ.is_incomplete() {
                        if v_typ.typ().deref(env) == Type::Primitive(PrimitiveType::Boolean) {
                            *o_typ = Type::Primitive(PrimitiveType::Boolean);
                            true
                        } else {
                            *o_typ = Type::Error(format!(
                                "Cannot negate non-boolean type {:?}",
                                v_typ.typ().deref(env)
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
            LogicalOp::And(left, right) => {
                if let TypedLogicalOp::And(left_typ, right_typ, o_typ) = typ {
                    let in_updated =
                        left.pass(left_typ, env, scope) || right.pass(right_typ, env, scope);
                    let out_updated = if o_typ.is_incomplete() {
                        if left_typ.typ().deref(env) == Type::Primitive(PrimitiveType::Boolean)
                            && right_typ.typ().deref(env) == Type::Primitive(PrimitiveType::Boolean)
                        {
                            *o_typ = Type::Primitive(PrimitiveType::Boolean);
                            true
                        } else {
                            *o_typ = Type::Error("Both inputs to && must be booleans".to_string());
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
                    let in_updated =
                        left.pass(left_typ, env, scope) || right.pass(right_typ, env, scope);
                    let out_updated = if o_typ.is_incomplete() {
                        if left_typ.typ().deref(env) == Type::Primitive(PrimitiveType::Boolean)
                            && right_typ.typ().deref(env) == Type::Primitive(PrimitiveType::Boolean)
                        {
                            *o_typ = Type::Primitive(PrimitiveType::Boolean);
                            true
                        } else {
                            *o_typ = Type::Error("Both inputs to || must be booleans".to_string());
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
