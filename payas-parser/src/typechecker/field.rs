use std::collections::HashMap;

use codemap_diagnostic::{Diagnostic, Level, SpanLabel, SpanStyle};
use payas_model::model::mapped_arena::MappedArena;

use crate::ast::ast_types::{
    AstExpr, AstField, AstFieldDefault, AstFieldDefaultKind, AstFieldType, Untyped,
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

        let default_value_types_changed = match &self.default_value {
            Some(AstFieldDefault {
                kind: AstFieldDefaultKind::Function(fn_name, _),
                ..
            }) => match fn_name.as_str() {
                "now" => match self.typ.name().as_str() {
                    "Instant" | "LocalDate" | "LocalTime" | "LocalDateTime" => false,

                    _ => {
                        errors.push(Diagnostic {
                            level: Level::Error,
                            message: "now() can only be used for time-related types".to_string(),
                            code: Some("C000".to_string()),
                            spans: vec![SpanLabel {
                                span: self.span,
                                style: SpanStyle::Primary,
                                label: None,
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
                            message: "autoincrement() can only be used on Ints".to_string(),
                            code: Some("C000".to_string()),
                            spans: vec![SpanLabel {
                                span: self.span,
                                style: SpanStyle::Primary,
                                label: None,
                            }],
                        });

                        true
                    }
                },

                _ => false,
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

                        true
                    } else {
                        false
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

                        true
                    }
                }
            }

            _ => false,
        };

        typ_changed || annot_changed || default_value_changed || default_value_types_changed
    }
}
