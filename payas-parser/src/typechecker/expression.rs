use std::collections::HashMap;

use codemap_diagnostic::Diagnostic;
use payas_model::model::mapped_arena::MappedArena;

use crate::ast::ast_types::{AstExpr, FieldSelection, LogicalOp, RelationalOp, Untyped};

use super::annotation::AnnotationSpec;
use super::{PrimitiveType, Scope, Type, TypecheckFrom, Typed};

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

impl TypecheckFrom<AstExpr<Untyped>> for AstExpr<Typed> {
    fn shallow(untyped: &AstExpr<Untyped>) -> AstExpr<Typed> {
        match untyped {
            AstExpr::FieldSelection(select) => {
                AstExpr::FieldSelection(FieldSelection::shallow(select))
            }
            AstExpr::LogicalOp(logic) => AstExpr::LogicalOp(LogicalOp::shallow(logic)),
            AstExpr::RelationalOp(relation) => {
                AstExpr::RelationalOp(RelationalOp::shallow(relation))
            }
            AstExpr::StringLiteral(v, s) => AstExpr::StringLiteral(v.clone(), *s),
            AstExpr::BooleanLiteral(v, s) => AstExpr::BooleanLiteral(*v, *s),
            AstExpr::NumberLiteral(v, s) => AstExpr::NumberLiteral(*v, *s),
        }
    }

    fn pass(
        &mut self,
        type_env: &MappedArena<Type>,
        annotation_env: &HashMap<String, AnnotationSpec>,
        scope: &Scope,
        errors: &mut Vec<Diagnostic>,
    ) -> bool {
        match self {
            AstExpr::FieldSelection(select) => select.pass(type_env, annotation_env, scope, errors),
            AstExpr::LogicalOp(logic) => logic.pass(type_env, annotation_env, scope, errors),
            AstExpr::RelationalOp(relation) => {
                relation.pass(type_env, annotation_env, scope, errors)
            }
            AstExpr::StringLiteral(_, _)
            | AstExpr::BooleanLiteral(_, _)
            | AstExpr::NumberLiteral(_, _) => false,
        }
    }
}
