use std::collections::HashMap;

use payas_model::model::mapped_arena::MappedArena;
use serde::{Deserialize, Serialize};

use crate::ast::ast_types::AstAnnotation;

use super::{Scope, Type, Typecheck, TypedExpression};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TypedAnnotation {
    pub name: String,
    pub params: HashMap<String, TypedExpression>,
}

impl TypedAnnotation {
    pub fn get_single_value(&self) -> Option<&TypedExpression> {
        if self.params.len() != 1 {
            None
        } else {
            self.params.get("value")
        }
    }
}

impl Typecheck<TypedAnnotation> for AstAnnotation {
    fn shallow(&self) -> TypedAnnotation {
        TypedAnnotation {
            name: self.name.to_string(),
            params: self
                .params
                .clone()
                .into_iter()
                .map(|(name, expr)| (name, expr.shallow()))
                .collect(),
        }
    }

    fn pass(
        &self,
        typ: &mut TypedAnnotation,
        env: &MappedArena<Type>,
        scope: &Scope,
        errors: &mut Vec<codemap_diagnostic::Diagnostic>,
    ) -> bool {
        let params_changed = self
            .params
            .iter()
            .map(|(name, expr)| {
                let typed_expr = typ.params.get_mut(name).unwrap();
                (name, expr.pass(typed_expr, env, scope, errors))
            })
            .filter(|(_, changed)| *changed)
            .count()
            > 0;
        params_changed
    }
}
