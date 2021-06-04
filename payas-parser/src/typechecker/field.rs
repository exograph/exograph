use payas_model::model::mapped_arena::MappedArena;
use serde::{Deserialize, Serialize};

use crate::ast::ast_types::AstField;

use super::{Scope, Type, Typecheck, TypedAnnotation};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TypedField {
    pub name: String,
    pub typ: Type,
    pub annotations: Vec<TypedAnnotation>,
}

impl TypedField {
    pub fn get_annotation(&self, name: &str) -> Option<&TypedAnnotation> {
        self.annotations.iter().find(|a| a.name == *name)
    }
}

impl Typecheck<TypedField> for AstField {
    fn shallow(&self) -> TypedField {
        TypedField {
            name: self.name.clone(),
            typ: self.typ.shallow(),
            annotations: self.annotations.iter().map(|a| a.shallow()).collect(),
        }
    }

    fn pass(&self, typ: &mut TypedField, env: &MappedArena<Type>, scope: &Scope) -> bool {
        let typ_changed = self.typ.pass(&mut typ.typ, env, scope);

        let annot_changed = self
            .annotations
            .iter()
            .zip(typ.annotations.iter_mut())
            .map(|(f, tf)| f.pass(tf, env, scope))
            .filter(|v| *v)
            .count()
            > 0;

        typ_changed || annot_changed
    }
}
