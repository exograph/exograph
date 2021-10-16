use std::collections::{HashMap, HashSet};

use crate::ast::ast_types::{AstAnnotation, AstAnnotationParams, Untyped};
use codemap_diagnostic::{Diagnostic, Level, SpanLabel, SpanStyle};
use payas_model::model::mapped_arena::MappedArena;

use super::{Scope, Type, TypecheckFrom, Typed};
use crate::util;

/// Specification for an annotation.
pub struct AnnotationSpec {
    /// List of targets the annotation is allowed to be applied to.
    pub targets: &'static [AnnotationTarget],
    /// Is this annotation allowed to have no parameters?
    pub no_params: bool,
    /// Is this annotation allowed to have a single parameter?
    pub single_params: bool,
    /// List of mapped parameters if mapped parameters are allowed (`None` if not).
    pub mapped_params: Option<&'static [MappedAnnotationParamSpec]>,
}

/// Target for an annotation.
#[derive(Debug, PartialEq)]
pub enum AnnotationTarget {
    Model,
    Field,
    Argument,
    Service,
    Method,
}

/// Specification for a mapped parameter of an annotation.
pub struct MappedAnnotationParamSpec {
    /// Name of the parameter.
    pub name: &'static str,
    /// Is this parameter optional?
    pub optional: bool,
}

impl TypecheckFrom<AstAnnotation<Untyped>> for AstAnnotation<Typed> {
    fn shallow(untyped: &AstAnnotation<Untyped>) -> AstAnnotation<Typed> {
        AstAnnotation {
            name: untyped.name.clone(),
            params: AstAnnotationParams::shallow(&untyped.params),
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
        let spec = &annotation_env[&self.name];

        // Message for what parameters were expected in case of a type error
        let diagnostic_msg = {
            let mut expected = Vec::new();

            if spec.no_params {
                expected.push("no parameters".to_string());
            }
            if spec.single_params {
                expected.push("a single parameter".to_string());
            }
            if let Some(params) = spec.mapped_params {
                expected.push(format!(
                    "({})",
                    util::join_strings(
                        &params
                            .iter()
                            .map(|param_spec| format!(
                                "{}{}",
                                param_spec.name,
                                if param_spec.optional { "?" } else { "" }
                            ))
                            .collect::<Vec<_>>(),
                        None,
                    )
                ));
            }

            format!("expected {}", util::join_strings(&expected, Some("or")))
        };

        let base_diagnostic = Diagnostic {
            level: Level::Error,
            message: format!("Incorrect parameters for annotation `{}`", self.name),
            code: Some("A000".to_string()),
            spans: vec![SpanLabel {
                span: self.span,
                label: Some(diagnostic_msg),
                style: SpanStyle::Primary,
            }],
        };

        match &self.params {
            AstAnnotationParams::None => {
                // Given no parameters, but expected single or mapped
                if !spec.no_params {
                    errors.push(base_diagnostic);
                }
            }
            AstAnnotationParams::Single(_, span) => {
                // Given a single parameter, but expected none or mapped
                if !spec.single_params {
                    let mut diagnostic = base_diagnostic;
                    diagnostic.spans.push(SpanLabel {
                        span: *span,
                        label: Some("unexpected parameter".to_owned()),
                        style: SpanStyle::Secondary,
                    });
                    errors.push(diagnostic);
                }
            }
            AstAnnotationParams::Map(params, spans) => match spec.mapped_params {
                // Given mapped parameters, but expected none or some
                None => errors.push(base_diagnostic),

                // Check given parameters are correct
                Some(param_specs) => {
                    let mut span_labels = Vec::new();
                    let mut missing_param = false;

                    // Check for any duplicate parameters
                    for (name, spans) in spans.iter() {
                        if spans.len() > 1 {
                            let mut span_labels = vec![SpanLabel {
                                span: spans[0],
                                label: Some("previously defined here".to_owned()),
                                style: SpanStyle::Secondary,
                            }];

                            for span in &spans[1..] {
                                span_labels.push(SpanLabel {
                                    span: *span,
                                    label: Some("redefined here".to_owned()),
                                    style: SpanStyle::Primary,
                                });
                            }

                            errors.push(Diagnostic {
                                level: Level::Error,
                                message: format!("Duplicate definitions of parameter `{}`", name),
                                code: Some("A000".to_string()),
                                spans: span_labels,
                            });
                        }
                    }

                    // Keep track of extra unused parameters
                    let mut unexpected_params = params.keys().cloned().collect::<HashSet<_>>();

                    // For each field, check if it is given or if it's optional
                    for param_spec in param_specs {
                        if params.contains_key(param_spec.name) {
                            unexpected_params.remove(param_spec.name);
                        } else if !param_spec.optional {
                            missing_param = true;
                        }
                    }

                    // For any unexpected parameters, push an error
                    for unexpected in unexpected_params {
                        span_labels.push(SpanLabel {
                            span: *spans[&unexpected].first().unwrap(),
                            label: Some("unexpected parameter".to_owned()),
                            style: SpanStyle::Secondary,
                        });
                    }

                    if !span_labels.is_empty() || missing_param {
                        let mut diagnostic = base_diagnostic;
                        diagnostic.spans.append(&mut span_labels);
                        errors.push(diagnostic);
                    }
                }
            },
        }

        self.params.pass(type_env, annotation_env, scope, errors)
    }
}
