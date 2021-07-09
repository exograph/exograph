use anyhow::Result;
use payas_model::model::mapped_arena::MappedArena;
use serde::{Deserialize, Serialize};

use crate::ast::ast_types::AstField;

use super::{AnnotationMap, Scope, Type, Typecheck};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TypedField {
    pub name: String,
    pub typ: Type,
    pub annotations: Box<AnnotationMap>,
}

impl Typecheck<TypedField> for AstField {
    fn shallow(&self, errors: &mut Vec<codemap_diagnostic::Diagnostic>) -> Result<TypedField> {
        let mut annotations = Box::new(AnnotationMap::default());

        for a in &self.annotations {
            annotations.add_annotation(a.shallow(errors)?);
        }

        Ok(TypedField {
            name: self.name.clone(),
            typ: self.typ.shallow(errors)?,
            annotations,
        })
    }

    fn pass(
        &self,
        typ: &mut TypedField,
        env: &MappedArena<Type>,
        scope: &Scope,
        errors: &mut Vec<codemap_diagnostic::Diagnostic>,
    ) -> bool {
        let typ_changed = self.typ.pass(&mut typ.typ, env, scope, errors);

        let annot_changed = typ.annotations.pass(&self.annotations, env, scope, errors);

        typ_changed || annot_changed
    }
}
