use anyhow::Result;
use payas_model::model::mapped_arena::MappedArena;

use crate::ast::ast_types::{AstExpr, Untyped};

use super::{PrimitiveType, Scope, Type, Typecheck, Typed};

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

impl Typecheck<AstExpr<Typed>> for AstExpr<Untyped> {
    fn shallow(&self, errors: &mut Vec<codemap_diagnostic::Diagnostic>) -> Result<AstExpr<Typed>> {
        Ok(match &self {
            AstExpr::FieldSelection(select) => AstExpr::FieldSelection(select.shallow(errors)?),
            AstExpr::LogicalOp(logic) => AstExpr::LogicalOp(logic.shallow(errors)?),
            AstExpr::RelationalOp(relation) => AstExpr::RelationalOp(relation.shallow(errors)?),
            AstExpr::StringLiteral(v, s) => AstExpr::StringLiteral(v.clone(), *s),
            AstExpr::BooleanLiteral(v, s) => AstExpr::BooleanLiteral(*v, *s),
            AstExpr::NumberLiteral(v, s) => AstExpr::NumberLiteral(*v, *s),
        })
    }

    fn pass(
        &self,
        typ: &mut AstExpr<Typed>,
        env: &MappedArena<Type>,
        scope: &Scope,
        errors: &mut Vec<codemap_diagnostic::Diagnostic>,
    ) -> bool {
        match &self {
            AstExpr::FieldSelection(select) => {
                if let AstExpr::FieldSelection(select_typ) = typ {
                    select.pass(select_typ, env, scope, errors)
                } else {
                    panic!()
                }
            }
            AstExpr::LogicalOp(logic) => {
                if let AstExpr::LogicalOp(logic_typ) = typ {
                    logic.pass(logic_typ, env, scope, errors)
                } else {
                    panic!("type {:?}", typ);
                }
            }
            AstExpr::RelationalOp(relation) => {
                if let AstExpr::RelationalOp(relation_typ) = typ {
                    relation.pass(relation_typ, env, scope, errors)
                } else {
                    panic!()
                }
            }
            AstExpr::StringLiteral(_, _)
            | AstExpr::BooleanLiteral(_, _)
            | AstExpr::NumberLiteral(_, _) => false,
        }
    }
}
