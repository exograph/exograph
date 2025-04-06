// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::collections::HashMap;

use codemap_diagnostic::{Diagnostic, Level, SpanLabel, SpanStyle};
use core_model::mapped_arena::MappedArena;
use core_model_builder::{
    ast::ast_types::{AstExpr, AstModel, FieldSelectionElement},
    typechecker::{annotation::AnnotationSpec, Typed},
};

use crate::ast::ast_types::{AstModelKind, FieldSelection, Untyped};

use super::{Scope, Type, TypecheckFrom};

pub trait TypecheckFunctionCallFrom<T>
where
    Self: Sized,
{
    fn shallow(untyped: &T) -> Self;
    fn pass(
        &mut self,
        type_env: &MappedArena<Type>,
        annotation_env: &HashMap<String, AnnotationSpec>,
        scope: &Scope,
        elem_type: Option<&Type>,
        errors: &mut Vec<Diagnostic>,
    ) -> bool;
}

impl TypecheckFunctionCallFrom<FieldSelectionElement<Untyped>> for FieldSelectionElement<Typed> {
    fn shallow(untyped: &FieldSelectionElement<Untyped>) -> Self {
        match untyped {
            FieldSelectionElement::Identifier(value, s, _) => {
                FieldSelectionElement::Identifier(value.clone(), *s, Type::Defer)
            }
            FieldSelectionElement::HofCall {
                span,
                name,
                param_name: elem_name,
                expr,
                ..
            } => FieldSelectionElement::HofCall {
                span: *span,
                name: name.clone(),
                param_name: elem_name.clone(),
                expr: Box::new(AstExpr::shallow(expr)),
                typ: Type::Defer,
            },
            FieldSelectionElement::NormalCall {
                span, name, params, ..
            } => FieldSelectionElement::NormalCall {
                span: *span,
                name: name.clone(),
                params: params.iter().map(AstExpr::shallow).collect(),
                typ: Type::Defer,
            },
        }
    }

    fn pass(
        &mut self,
        type_env: &MappedArena<Type>,
        annotation_env: &HashMap<String, AnnotationSpec>,
        scope: &Scope,
        elem_type: Option<&Type>,
        errors: &mut Vec<Diagnostic>,
    ) -> bool {
        match self {
            FieldSelectionElement::Identifier(value, s, typ) => {
                if typ.is_incomplete() {
                    match scope.get_type(value) {
                        Some(type_name) => {
                            *typ = Type::Reference(type_env.get_id(type_name).unwrap());
                            true
                        }
                        None => {
                            if value.as_str() == "self" {
                                *typ = Type::Error;

                                errors.push(Diagnostic {
                                    level: Level::Error,
                                    message: "Cannot use self outside a model".to_string(),
                                    code: Some("C000".to_string()),
                                    spans: vec![SpanLabel {
                                        span: *s,
                                        style: SpanStyle::Primary,
                                        label: Some("self not allowed".to_string()),
                                    }],
                                });

                                false
                            } else {
                                let context_type =
                                    type_env.get_by_key(value).and_then(|t| match t {
                                        Type::Composite(c) if c.kind == AstModelKind::Context => {
                                            Some(c)
                                        }
                                        _ => None,
                                    });

                                if let Some(context_type) = context_type {
                                    *typ = Type::Reference(
                                        type_env.get_id(&context_type.name).unwrap(),
                                    );
                                    true
                                } else {
                                    *typ = Type::Error;

                                    errors.push(Diagnostic {
                                        level: Level::Error,
                                        message: format!("Reference to unknown context: {value}"),
                                        code: Some("C000".to_string()),
                                        spans: vec![SpanLabel {
                                            span: *s,
                                            style: SpanStyle::Primary,
                                            label: Some("unknown context".to_string()),
                                        }],
                                    });
                                    false
                                }
                            }
                        }
                    }
                } else {
                    false
                }
            }
            FieldSelectionElement::HofCall {
                param_name: elem_name,
                expr,
                typ,
                ..
            } => {
                let function_scope = scope.with_additional_mapping(HashMap::from_iter([(
                    elem_name.0.clone(),
                    elem_type
                        .and_then(|t| t.get_underlying_typename(type_env))
                        .unwrap(),
                )]));
                let updated = expr.pass(type_env, annotation_env, &function_scope, errors);
                *typ = expr.typ().clone();
                updated
            }
            FieldSelectionElement::NormalCall {
                name,
                params,
                typ,
                span,
            } => {
                let updated = params
                    .iter_mut()
                    .map(|p| p.pass(type_env, annotation_env, scope, errors))
                    .all(|b| b);

                if name.0 != "contains" {
                    *typ = Type::Error;

                    errors.push(Diagnostic {
                        level: Level::Error,
                        message: format!(
                            "Only `contains` function is supported on arrays, not `{}`",
                            name.0
                        ),
                        code: Some("C000".to_string()),
                        spans: vec![SpanLabel {
                            span: *span,
                            style: SpanStyle::Primary,
                            label: Some("unknown context".to_string()),
                        }],
                    });
                    false
                } else {
                    // Ensure that the params and elem_type are compatible
                    if params.is_empty() {
                        *typ = Type::Error;
                        errors.push(Diagnostic {
                            level: Level::Error,
                            message: "Contains function expects one parameter".to_string(),
                            code: Some("C000".to_string()),
                            spans: vec![SpanLabel {
                                span: *span,
                                style: SpanStyle::Primary,
                                label: Some("type mismatch".to_string()),
                            }],
                        });
                        return false;
                    }
                    if params.len() == 1 {
                        let param_type = params[0].typ();
                        if let Some(elem_type) = elem_type {
                            if elem_type != &param_type {
                                *typ = Type::Error;
                                errors.push(Diagnostic {
                                    level: Level::Error,
                                    message: format!(
                                        "Parameter type does not match element type: expected `{}`, found `{}`",
                                        elem_type, param_type,
                                    ),
                                    code: Some("C000".to_string()),
                                    spans: vec![SpanLabel {
                                        span: params[0].span(),
                                        style: SpanStyle::Primary,
                                        label: Some("type mismatch".to_string()),
                                    }],
                                });
                                return false;
                            }
                        }
                    }
                    *typ = typ.clone();
                    updated
                }
            }
        }
    }
}

impl TypecheckFrom<FieldSelection<Untyped>> for FieldSelection<Typed> {
    fn shallow(untyped: &FieldSelection<Untyped>) -> FieldSelection<Typed> {
        match untyped {
            FieldSelection::Single(v, _) => {
                FieldSelection::Single(FieldSelectionElement::shallow(v), Type::Defer)
            }
            FieldSelection::Select(selection, i, span, _) => FieldSelection::Select(
                Box::new(FieldSelection::shallow(selection)),
                FieldSelectionElement::shallow(i),
                *span,
                Type::Defer,
            ),
        }
    }

    fn pass(
        &mut self,
        type_env: &MappedArena<Type>,
        annotation_env: &HashMap<String, AnnotationSpec>,
        scope: &Scope,
        errors: &mut Vec<Diagnostic>,
    ) -> bool {
        match self {
            FieldSelection::Single(selection_elem, typ) => {
                let updated = selection_elem.pass(type_env, annotation_env, scope, None, errors);
                match selection_elem {
                    FieldSelectionElement::Identifier(_, _, resolved_typ) => {
                        *typ = resolved_typ.clone();
                    }
                    FieldSelectionElement::HofCall { name, span, .. }
                    | FieldSelectionElement::NormalCall { name, span, .. } => {
                        *typ = Type::Error;
                        errors.push(Diagnostic {
                            level: Level::Error,
                            message: format!(
                                "Function call cannot be a top-level field selection: {}",
                                name.0
                            ),
                            code: Some("C000".to_string()),
                            spans: vec![SpanLabel {
                                span: *span,
                                style: SpanStyle::Primary,
                                label: Some(
                                    "Function call cannot be a top-level field selection"
                                        .to_string(),
                                ),
                            }],
                        });
                    }
                }
                updated
            }
            FieldSelection::Select(prefix, elem, _, typ) => {
                let in_updated = prefix.pass(type_env, annotation_env, scope, errors);
                let out_updated = if typ.is_incomplete() {
                    match prefix.typ().deref(type_env) {
                        Type::Optional(elem_type) => {
                            if let Type::Composite(c) = *elem_type {
                                process_composite_type_selection(elem, &c, typ, type_env, errors)
                            } else {
                                // Support optional field selection by calling pass on the element. This
                                // uniformly dealing with simple selection and hof calls on optional
                                // fields
                                let updated = elem.pass(
                                    type_env,
                                    annotation_env,
                                    scope,
                                    Some(&elem_type),
                                    errors,
                                );
                                *typ = elem.typ().clone();
                                updated
                            }
                        }
                        Type::Composite(c) => {
                            process_composite_type_selection(elem, &c, typ, type_env, errors)
                        }
                        Type::Set(elem_type) => match elem {
                            FieldSelectionElement::Identifier(value, span, _) => {
                                *typ = Type::Error;
                                errors.push(Diagnostic {
                                        level: Level::Error,
                                        message: format!(
                                            "Plain field selection '{value}' not supported on set type {elem_type}"
                                        ),
                                        code: Some("C000".to_string()),
                                        spans: vec![SpanLabel {
                                            span: *span,
                                            style: SpanStyle::Primary,
                                            label: Some("unsupported field".to_string()),
                                        }],
                                    });
                                return false;
                            }
                            hof_call @ FieldSelectionElement::HofCall { .. } => {
                                let updated = hof_call.pass(
                                    type_env,
                                    annotation_env,
                                    scope,
                                    Some(&elem_type),
                                    errors,
                                );
                                *typ = hof_call.typ().clone();
                                updated
                            }
                            normal_call @ FieldSelectionElement::NormalCall { .. } => {
                                let updated = normal_call.pass(
                                    type_env,
                                    annotation_env,
                                    scope,
                                    Some(&elem_type),
                                    errors,
                                );
                                *typ = normal_call.typ().clone();
                                updated
                            }
                        },
                        Type::Array(elem_type) => match elem {
                            normal_call @ FieldSelectionElement::NormalCall { .. } => {
                                let updated = normal_call.pass(
                                    type_env,
                                    annotation_env,
                                    scope,
                                    Some(&elem_type),
                                    errors,
                                );
                                *typ = normal_call.typ().clone();
                                updated
                            }
                            _ => {
                                *typ = Type::Error;

                                errors.push(Diagnostic {
                                    level: Level::Error,
                                    message: "Only `contains` function is supported on arrays"
                                        .to_string(),
                                    code: Some("C000".to_string()),
                                    spans: vec![SpanLabel {
                                        span: *elem.span(),
                                        style: SpanStyle::Primary,
                                        label: Some("unsupported field".to_string()),
                                    }],
                                });
                                false
                            }
                        },
                        _ => {
                            *typ = Type::Error;

                            let field_name = match elem {
                                FieldSelectionElement::Identifier(value, _, _) => value,
                                FieldSelectionElement::HofCall { name, .. } => &name.0,
                                FieldSelectionElement::NormalCall { name, .. } => &name.0,
                            };

                            if !prefix.typ().is_error() {
                                errors.push(Diagnostic {
                                    level: Level::Error,
                                    message: format!(
                                        "Cannot read field `{}` from a non-composite type `{}`",
                                        field_name,
                                        prefix.typ().deref(type_env)
                                    ),
                                    code: Some("C000".to_string()),
                                    spans: vec![SpanLabel {
                                        span: *prefix.span(),
                                        style: SpanStyle::Primary,
                                        label: Some("non-composite value".to_string()),
                                    }],
                                });
                            }

                            false
                        }
                    }
                } else {
                    false
                };

                in_updated || out_updated
            }
        }
    }
}

fn process_composite_type_selection(
    elem: &mut FieldSelectionElement<Typed>,
    composite_type_model: &AstModel<Typed>,
    typ: &mut Type,
    type_env: &MappedArena<Type>,
    errors: &mut Vec<Diagnostic>,
) -> bool {
    let elem = match elem {
        FieldSelectionElement::Identifier(value, s, _) => (value, *s),
        FieldSelectionElement::HofCall { span, name, .. } => {
            *typ = Type::Error;
            errors.push(Diagnostic {
                level: Level::Error,
                message: format!(
                    "Function call {} not supported on type {}",
                    name.0, composite_type_model.name
                ),
                code: Some("C000".to_string()),
                spans: vec![SpanLabel {
                    span: *span,
                    style: SpanStyle::Primary,
                    label: Some("unsupported function call target type".to_string()),
                }],
            });
            return false;
        }
        FieldSelectionElement::NormalCall {
            span, name, typ, ..
        } => {
            *typ = Type::Error;
            errors.push(Diagnostic {
                level: Level::Error,
                message: format!(
                    "Function call {} not supported on type {}",
                    name.0, composite_type_model.name
                ),
                code: Some("C000".to_string()),
                spans: vec![SpanLabel {
                    span: *span,
                    style: SpanStyle::Primary,
                    label: Some("unsupported function call target type".to_string()),
                }],
            });
            return false;
        }
    };

    let frgamgnt_fields = composite_type_model
        .fragment_references
        .iter()
        .flat_map(
            |fragment_reference| match type_env.get_by_key(&fragment_reference.name) {
                Some(fragment_type) => match fragment_type.deref(type_env) {
                    Type::Composite(c) => c.fields,
                    _ => vec![],
                },
                None => {
                    *typ = Type::Error;
                    errors.push(Diagnostic {
                        level: Level::Error,
                        message: format!("No such fragment: {}", fragment_reference.name),
                        code: Some("C000".to_string()),
                        spans: vec![SpanLabel {
                            span: fragment_reference.span,
                            style: SpanStyle::Primary,
                            label: Some("unknown field".to_string()),
                        }],
                    });
                    vec![]
                }
            },
        )
        .collect::<Vec<_>>();

    if let Some(field) = composite_type_model
        .fields
        .iter()
        .chain(frgamgnt_fields.iter())
        .find(|f| &f.name == elem.0)
    {
        let resolved_typ = field.typ.to_typ(type_env);
        if resolved_typ.is_complete() {
            *typ = resolved_typ;
            true
        } else {
            *typ = Type::Error;
            // no diagnostic because the prefix is incomplete
            false
        }
    } else {
        *typ = Type::Error;
        errors.push(Diagnostic {
            level: Level::Error,
            message: format!(
                "No such field {} on type {}",
                elem.0, composite_type_model.name
            ),
            code: Some("C000".to_string()),
            spans: vec![SpanLabel {
                span: elem.1,
                style: SpanStyle::Primary,
                label: Some("unknown field".to_string()),
            }],
        });
        false
    }
}
