use std::collections::HashMap;

use codemap_diagnostic::{Diagnostic, Level, SpanLabel, SpanStyle};
use payas_core_model::mapped_arena::MappedArena;
use payas_core_model_builder::typechecker::{
    annotation::{AnnotationSpec, AnnotationTarget},
    annotation_map::AnnotationMap,
    Typed,
};

use crate::ast::ast_types::{AstField, AstFieldDefault, AstModel, AstModelKind, Untyped};

use super::{annotation_map::AnnotationMapImpl, Scope, Type, TypecheckFrom};

impl TypecheckFrom<AstModel<Untyped>> for AstModel<Typed> {
    fn shallow(untyped: &AstModel<Untyped>) -> AstModel<Typed> {
        let annotation_map = AnnotationMap::new(&untyped.annotations);

        AstModel {
            name: untyped.name.clone(),
            kind: untyped.kind.clone(),
            fields: untyped.fields.iter().map(AstField::shallow).collect(),
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

        match self.kind {
            AstModelKind::Persistent => {}
            AstModelKind::Context
            | AstModelKind::NonPersistent
            | AstModelKind::NonPersistentInput => self
                .fields
                .iter()
                .map(|field| {
                    if let Some(AstFieldDefault { span, .. }) = &field.default_value {
                        errors.push(Diagnostic {
                            level: Level::Error,
                            message: "Default fields can only be specified in models".to_string(),
                            code: Some("C000".to_string()),
                            spans: vec![SpanLabel {
                                span: *span,
                                style: SpanStyle::Primary,
                                label: Some("bad default field".to_string()),
                            }],
                        });
                    }
                })
                .collect(),
        };

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
