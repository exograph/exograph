use payas_model::model::mapped_arena::MappedArena;
use serde::{Deserialize, Serialize};

use crate::ast::ast_types::AstExpr;

use super::{
    logical_op::TypedLogicalOp, relational_op::TypedRelationalOp, selection::TypedFieldSelection,
    PrimitiveType, Scope, Type, Typecheck,
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TypedExpression {
    FieldSelection(TypedFieldSelection),
    LogicalOp(TypedLogicalOp),
    RelationalOp(TypedRelationalOp),
    StringLiteral(String, Type),
    BooleanLiteral(bool, Type),
}

impl TypedExpression {
    pub fn typ(&self) -> &Type {
        match &self {
            TypedExpression::FieldSelection(select) => select.typ(),
            TypedExpression::LogicalOp(logic) => logic.typ(),
            TypedExpression::RelationalOp(relation) => relation.typ(),
            TypedExpression::StringLiteral(_, t) => t,
            TypedExpression::BooleanLiteral(_, t) => t,
        }
    }

    pub fn as_string(&self) -> String {
        match &self {
            TypedExpression::StringLiteral(s, _) => s.clone(),
            _ => panic!(),
        }
    }
}

impl Typecheck<TypedExpression> for AstExpr {
    fn shallow(&self) -> TypedExpression {
        match &self {
            AstExpr::FieldSelection(select) => TypedExpression::FieldSelection(select.shallow()),
            AstExpr::LogicalOp(logic) => TypedExpression::LogicalOp(logic.shallow()),
            AstExpr::RelationalOp(relation) => TypedExpression::RelationalOp(relation.shallow()),
            AstExpr::StringLiteral(v, _) => {
                TypedExpression::StringLiteral(v.clone(), Type::Primitive(PrimitiveType::String))
            }
            AstExpr::BooleanLiteral(v, _) => {
                TypedExpression::BooleanLiteral(v.clone(), Type::Primitive(PrimitiveType::Boolean))
            }
        }
    }

    fn pass(
        &self,
        typ: &mut TypedExpression,
        env: &MappedArena<Type>,
        scope: &Scope,
        errors: &mut Vec<codemap_diagnostic::Diagnostic>,
    ) -> bool {
        match &self {
            AstExpr::FieldSelection(select) => {
                if let TypedExpression::FieldSelection(select_typ) = typ {
                    select.pass(select_typ, env, scope, errors)
                } else {
                    panic!()
                }
            }
            AstExpr::LogicalOp(logic) => {
                if let TypedExpression::LogicalOp(logic_typ) = typ {
                    logic.pass(logic_typ, env, scope, errors)
                } else {
                    panic!()
                }
            }
            AstExpr::RelationalOp(relation) => {
                if let TypedExpression::RelationalOp(relation_typ) = typ {
                    relation.pass(relation_typ, env, scope, errors)
                } else {
                    panic!()
                }
            }
            AstExpr::StringLiteral(_, _) | AstExpr::BooleanLiteral(_, _) => false,
        }
    }
}
