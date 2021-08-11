use std::collections::HashMap;

use codemap_diagnostic::Diagnostic;
use payas_model::model::mapped_arena::MappedArena;

use crate::ast::ast_types::{AstField, AstModel, Untyped};

use super::annotation::{AnnotationSpec, AnnotationTarget};
use super::{AnnotationMap, Scope, Type, TypecheckFrom, Typed};

impl TypecheckFrom<AstModel<Untyped>> for AstModel<Typed> {
    fn shallow(untyped: &AstModel<Untyped>) -> AstModel<Typed> {
        let annotation_map = AnnotationMap::new(&untyped.annotations);

        AstModel {
            name: untyped.name.clone(),
            kind: untyped.kind.clone(),
            fields: untyped
                .fields
                .iter()
                .map(|f| AstField::shallow(f))
                .collect(),
            annotations: annotation_map,
        }
    }

    fn pass(
        &mut self,
        type_env: &MappedArena<Type>,
        annotation_env: &HashMap<String, AnnotationSpec>,
        _scope: &Scope,
        errors: &mut Vec<Diagnostic>,
    ) -> bool {
        let model_scope = Scope {
            enclosing_model: Some(self.name.clone()),
        };

        let fields_changed = self
            .fields
            .iter_mut()
            .map(|tf| tf.pass(type_env, annotation_env, &model_scope, errors))
            .filter(|v| *v)
            .count()
            > 0;

        let annot_changed = self.annotations.pass(
            AnnotationTarget::Model,
            type_env,
            annotation_env,
            &model_scope,
            errors,
        );

        fields_changed || annot_changed
    }
}
