use anyhow::Result;
use payas_model::model::mapped_arena::MappedArena;

use crate::ast::ast_types::{AstExpr, FieldSelection, LogicalOp, RelationalOp, Untyped};

use super::{PrimitiveType, Scope, Type, TypecheckNew, Typed};

static STR_TYP: Type = Type::Primitive(PrimitiveType::String);
static BOOL_TYP: Type = Type::Primitive(PrimitiveType::Boolean);
static INT_TYP: Type = Type::Primitive(PrimitiveType::Int);

impl AstExpr<Typed> {
    pub fn typ(&self) -> &Type {
        match &self {
            AstExpr::FieldSelection(select) => select.typ(),
            AstExpr::LogicalOp(logic) => logic.typ(),
            AstExpr::RelationalOp(relation) => relation.typ(),
            AstExpr::StringLiteral(_, _) => &STR_TYP,
            AstExpr::BooleanLiteral(_, _) => &BOOL_TYP,
            AstExpr::NumberLiteral(_, _) => &INT_TYP,
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

impl TypecheckNew<AstExpr<Untyped>> for AstExpr<Typed> {
    fn shallow(
        untyped: &AstExpr<Untyped>,
        errors: &mut Vec<codemap_diagnostic::Diagnostic>,
    ) -> Result<AstExpr<Typed>> {
        Ok(match untyped {
            AstExpr::FieldSelection(select) => {
                AstExpr::FieldSelection(FieldSelection::shallow(select, errors)?)
            }
            AstExpr::LogicalOp(logic) => AstExpr::LogicalOp(LogicalOp::shallow(logic, errors)?),
            AstExpr::RelationalOp(relation) => {
                AstExpr::RelationalOp(RelationalOp::shallow(relation, errors)?)
            }
            AstExpr::StringLiteral(v, s) => AstExpr::StringLiteral(v.clone(), *s),
            AstExpr::BooleanLiteral(v, s) => AstExpr::BooleanLiteral(*v, *s),
            AstExpr::NumberLiteral(v, s) => AstExpr::NumberLiteral(*v, *s),
        })
    }

    fn pass(
        &mut self,
        env: &MappedArena<Type>,
        scope: &Scope,
        errors: &mut Vec<codemap_diagnostic::Diagnostic>,
    ) -> bool {
        match self {
            AstExpr::FieldSelection(select) => select.pass(env, scope, errors),
            AstExpr::LogicalOp(logic) => logic.pass(env, scope, errors),
            AstExpr::RelationalOp(relation) => relation.pass(env, scope, errors),
            AstExpr::StringLiteral(_, _)
            | AstExpr::BooleanLiteral(_, _)
            | AstExpr::NumberLiteral(_, _) => false,
        }
    }
}
