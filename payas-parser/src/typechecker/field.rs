use std::collections::HashMap;

use codemap_diagnostic::{Diagnostic, Level, SpanLabel, SpanStyle};
use payas_core_model::mapped_arena::MappedArena;
use payas_core_model_builder::typechecker::annotation::{AnnotationSpec, AnnotationTarget};
use payas_core_model_builder::typechecker::annotation_map::AnnotationMap;
use payas_core_model_builder::typechecker::Typed;

use crate::ast::ast_types::{
    AstExpr, AstField, AstFieldDefault, AstFieldDefaultKind, AstFieldType, Untyped,
};
use payas_database_model_builder::builder::{
    DEFAULT_FN_AUTOINCREMENT, DEFAULT_FN_CURRENT_TIME, DEFAULT_FN_GENERATE_UUID,
};

use super::annotation_map::AnnotationMapImpl;
use super::{Scope, Type, TypecheckFrom};

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

        match &self.default_value {
            Some(AstFieldDefault {
                kind: AstFieldDefaultKind::Function(fn_name, _),
                ..
            }) => match fn_name.as_str() {
                DEFAULT_FN_CURRENT_TIME => match self.typ.name().as_str() {
                    "Instant" | "LocalDate" | "LocalTime" | "LocalDateTime" => {}

                    _ => {
                        errors.push(Diagnostic {
                            level: Level::Error,
                            message: format!(
                                "{}() can only be used for time-related types",
                                DEFAULT_FN_CURRENT_TIME
                            ),
                            code: Some("C000".to_string()),
                            spans: vec![SpanLabel {
                                span: self.span,
                                style: SpanStyle::Primary,
                                label: None,
                            }],
                        });
                    }
                },

                DEFAULT_FN_AUTOINCREMENT => match self.typ.name().as_str() {
                    "Int" => {}

                    _ => {
                        errors.push(Diagnostic {
                            level: Level::Error,
                            message: format!(
                                "{}() can only be used on Ints",
                                DEFAULT_FN_AUTOINCREMENT
                            ),
                            code: Some("C000".to_string()),
                            spans: vec![SpanLabel {
                                span: self.span,
                                style: SpanStyle::Primary,
                                label: None,
                            }],
                        });
                    }
                },

                DEFAULT_FN_GENERATE_UUID => match self.typ.name().as_str() {
                    "Uuid" => {}

                    _ => {
                        errors.push(Diagnostic {
                            level: Level::Error,
                            message: format!(
                                "{}() can only be used on Uuids",
                                DEFAULT_FN_GENERATE_UUID
                            ),
                            code: Some("C000".to_string()),
                            spans: vec![SpanLabel {
                                span: self.span,
                                style: SpanStyle::Primary,
                                label: None,
                            }],
                        });
                    }
                },

                _ => {}
            },

            Some(AstFieldDefault {
                kind: AstFieldDefaultKind::Value(expr),
                ..
            }) => {
                let type_name = self.typ.name();
                let mut assert_type = |types_allowed: &[&str]| {
                    if !types_allowed.contains(&type_name.as_str()) {
                        let types_allowed: String = types_allowed.join(", ");

                        errors.push(Diagnostic {
                            level: Level::Error,
                            message:
                                "Literal specified for default value is not a valid type for field."
                                    .to_string(),
                            code: Some("C000".to_string()),
                            spans: vec![SpanLabel {
                                span: *expr.span(),
                                style: SpanStyle::Primary,
                                label: Some(format!("should be of type {}", types_allowed)),
                            }],
                        });
                    }
                };

                match *expr {
                    AstExpr::StringLiteral(_, _) => assert_type(&["String"]),
                    AstExpr::BooleanLiteral(_, _) => assert_type(&["Boolean"]),
                    AstExpr::NumberLiteral(_, _) => assert_type(&["Int", "Float"]),

                    _ => {
                        errors.push(Diagnostic {
                            level: Level::Error,
                            message: "Non-literal specified in default value field.".to_string(),
                            code: Some("C000".to_string()),
                            spans: vec![SpanLabel {
                                span: *expr.span(),
                                style: SpanStyle::Primary,
                                label: Some("should be string, boolean, or a number".to_string()),
                            }],
                        });
                    }
                }
            }

            _ => {}
        };

        typ_changed || annot_changed || default_value_changed
    }
}
