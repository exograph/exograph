use payas_model::model::mapped_arena::MappedArena;
use serde::{Deserialize, Serialize};

use crate::ast::ast_types::AstAnnotation;

use super::{Scope, Type, Typecheck, TypedExpression};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TypedAnnotation {
    pub name: String,
    pub params: Vec<TypedExpression>,
}

impl Typecheck<TypedAnnotation> for AstAnnotation {
    fn shallow(&self) -> TypedAnnotation {
        TypedAnnotation {
            name: self.name.clone(),
            params: self.params.iter().map(|p| p.shallow()).collect(),
        }
    }

    fn pass(&self, typ: &mut TypedAnnotation, env: &MappedArena<Type>, scope: &Scope) -> bool {
        let params_changed = self
            .params
            .iter()
            .zip(typ.params.iter_mut())
            .map(|(p, p_typ)| p.pass(p_typ, env, scope))
            .filter(|c| *c)
            .count()
            > 0;
        params_changed
    }
}
