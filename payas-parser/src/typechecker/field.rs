use std::collections::HashMap;

use codemap_diagnostic::{Diagnostic, Level, SpanLabel, SpanStyle};
use payas_model::model::mapped_arena::MappedArena;

use crate::ast::ast_types::{
    AstField, AstFieldDefault, AstFieldDefaultKind, AstFieldType, Untyped,
};

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

        let default_value_types_changed = if let Some(AstFieldDefault {
            kind: AstFieldDefaultKind::Function(fn_name, _),
            ..
        }) = &self.default_value
        {
            match fn_name.as_str() {
                "now" => match self.typ.name().as_str() {
                    "Instant" | "LocalDate" | "LocalTime" | "LocalDateTime" => false,

                    _ => {
                        errors.push(Diagnostic {
                            level: Level::Error,
                            message: format!("now() can only be used for time-related types",),
                            code: Some("C000".to_string()),
                            spans: vec![SpanLabel {
                                span: self.span,
                                style: SpanStyle::Primary,
                                label: Some("field containing now()".to_string()),
                            }],
                        });

                        true
                    }
                },

                "autoincrement" => match self.typ.name().as_str() {
                    "Int" => false,

                    _ => {
                        errors.push(Diagnostic {
                            level: Level::Error,
                            message: format!("autoincrement() can only be used for Ints",),
                            code: Some("C000".to_string()),
                            spans: vec![SpanLabel {
                                span: self.span,
                                style: SpanStyle::Primary,
                                label: Some("field containing autoincrement()".to_string()),
                            }],
                        });

                        true
                    }
                },

                _ => false,
            }
        } else {
            false
        };

        typ_changed || annot_changed || default_value_changed || default_value_types_changed
    }
}
