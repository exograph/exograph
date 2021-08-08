use crate::ast::ast_types::AstAnnotation;
use anyhow::Result;
use payas_model::model::mapped_arena::MappedArena;
use serde::{Deserialize, Serialize};

use super::{annotation_params::TypedAnnotationParams, Scope, Type, Typecheck};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TypedAnnotation {
    pub name: String,
    pub params: TypedAnnotationParams,
}

impl Typecheck<TypedAnnotation> for AstAnnotation {
    fn shallow(&self, errors: &mut Vec<codemap_diagnostic::Diagnostic>) -> Result<TypedAnnotation> {
        Ok(TypedAnnotation {
            name: self.name.clone(),
            params: self.params.shallow(errors)?,
        })
    }

    fn pass(
        &self,
        typ: &mut TypedAnnotation,
        env: &MappedArena<Type>,
        scope: &Scope,
        errors: &mut Vec<codemap_diagnostic::Diagnostic>,
    ) -> bool {
        // TODO check name exists
        // TODO check params are correct format
        self.params.pass(&mut typ.params, env, scope, errors)
    }
}
