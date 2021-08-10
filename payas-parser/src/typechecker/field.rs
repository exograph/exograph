use std::collections::HashMap;

use payas_model::model::mapped_arena::MappedArena;
use serde::{Deserialize, Serialize};

use crate::ast::ast_types::AstField;

use super::annotation::AnnotationSpec;
use super::{AnnotationMap, Scope, Type, Typecheck};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TypedField {
    pub name: String,
    pub typ: Type,
    pub annotation_map: AnnotationMap,
}

impl Typecheck<TypedField> for AstField {
    fn shallow(&self) -> TypedField {
        let annotation_map = AnnotationMap::new(&self.annotations);

        TypedField {
            name: self.name.clone(),
            typ: self.typ.shallow(),
            annotation_map,
        }
    }

    fn pass(
        &self,
        typ: &mut TypedField,
        type_env: &MappedArena<Type>,
        annotation_env: &HashMap<String, AnnotationSpec>,
        scope: &Scope,
        errors: &mut Vec<codemap_diagnostic::Diagnostic>,
    ) -> bool {
        let typ_changed = self
            .typ
            .pass(&mut typ.typ, type_env, annotation_env, scope, errors);
        let annot_changed =
            typ.annotation_map
                .pass(&self.annotations, type_env, annotation_env, scope, errors);

        typ_changed || annot_changed
    }
}
