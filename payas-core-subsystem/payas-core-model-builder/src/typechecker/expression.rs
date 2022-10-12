use payas_core_model::primitive_type::PrimitiveType;

use crate::ast::ast_types::AstExpr;

use super::{Type, Typed};

impl AstExpr<Typed> {
    pub fn typ(&self) -> Type {
        match &self {
            AstExpr::FieldSelection(select) => select.typ().clone(),
            AstExpr::LogicalOp(logic) => logic.typ().clone(),
            AstExpr::RelationalOp(relation) => relation.typ().clone(),
            AstExpr::StringLiteral(_, _) => Type::Primitive(PrimitiveType::String),
            AstExpr::BooleanLiteral(_, _) => Type::Primitive(PrimitiveType::Boolean),
            AstExpr::NumberLiteral(_, _) => Type::Primitive(PrimitiveType::Int),
            AstExpr::StringList(_, _) => {
                Type::Array(Box::new(Type::Primitive(PrimitiveType::String)))
            }
        }
    }

    pub fn as_string(&self) -> String {
        match &self {
            AstExpr::StringLiteral(s, _) => s.clone(),
            _ => panic!(),
        }
    }

    pub fn as_number(&self) -> i64 {
        match &self {
            AstExpr::NumberLiteral(n, _) => *n,
            _ => panic!(),
        }
    }
}
