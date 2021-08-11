use std::collections::HashMap;

use codemap_diagnostic::Diagnostic;
use payas_model::model::mapped_arena::MappedArena;

use crate::ast::ast_types::{AstField, Untyped};

use super::annotation::AnnotationSpec;
use super::{AnnotationMap, Scope, Type, TypecheckFrom, TypecheckInto, Typed};

impl TypecheckFrom<AstField<Untyped>> for AstField<Typed> {
    fn shallow(untyped: &AstField<Untyped>) -> AstField<Typed> {
        let annotation_map = AnnotationMap::new(&untyped.annotations);

        AstField {
            name: untyped.name.clone(),
            ast_typ: untyped.ast_typ.clone(),
            typ: untyped.ast_typ.shallow(),
            annotations: annotation_map,
        }
    }

    fn pass(
        &mut self,
        type_env: &MappedArena<Type>,
        annotation_env: &HashMap<String, AnnotationSpec>,
        scope: &Scope,
        errors: &mut Vec<Diagnostic>,
    ) -> bool {
        let typ_changed = self
            .ast_typ
            .pass(&mut self.typ, type_env, annotation_env, scope, errors);

        let annot_changed = self
            .annotations
            .pass(type_env, annotation_env, scope, errors);

        typ_changed || annot_changed
    }
}
