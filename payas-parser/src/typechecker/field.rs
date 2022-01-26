use std::collections::HashMap;

use codemap_diagnostic::Diagnostic;
use payas_model::model::mapped_arena::MappedArena;

use crate::ast::ast_types::{AstField, AstFieldDefault, AstFieldType, Untyped};

use super::annotation::{AnnotationSpec, AnnotationTarget};
use super::{AnnotationMap, Scope, Type, TypecheckFrom, Typed};

impl TypecheckFrom<AstField<Untyped>> for AstField<Typed> {
    fn shallow(untyped: &AstField<Untyped>) -> AstField<Typed> {
        let annotation_map = AnnotationMap::new(&untyped.annotations);

        AstField {
            name: untyped.name.clone(),
            typ: AstFieldType::shallow(&untyped.typ),
            annotations: annotation_map,
            default_value: untyped.default_value.as_ref().map(AstFieldDefault::shallow),
            span: untyped.span,
        }
    }

    fn pass(
        &mut self,
        type_env: &MappedArena<Type>,
        annotation_env: &HashMap<String, AnnotationSpec>,
        scope: &Scope,
        errors: &mut Vec<Diagnostic>,
    ) -> bool {
        let typ_changed = self.typ.pass(type_env, annotation_env, scope, errors);

        let annot_changed = self.annotations.pass(
            AnnotationTarget::Field,
            type_env,
            annotation_env,
            scope,
            errors,
        );

        let default_value_changed = self
            .default_value
            .as_mut()
            .map(|default_value| default_value.pass(type_env, annotation_env, scope, errors))
            .unwrap_or(false);

        typ_changed || annot_changed || default_value_changed
    }
}
