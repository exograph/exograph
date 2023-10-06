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
    ast::ast_types::{AstExpr, FieldSelectionElement},
    typechecker::{annotation::AnnotationSpec, Typed},
};

use crate::ast::ast_types::{AstModelKind, FieldSelection, Untyped};

use super::{Scope, Type, TypecheckFrom};

impl TypecheckFrom<FieldSelectionElement<Untyped>> for FieldSelectionElement<Typed> {
    fn shallow(untyped: &FieldSelectionElement<Untyped>) -> Self {
        match untyped {
            FieldSelectionElement::Identifier(i, s, _) => {
                FieldSelectionElement::Identifier(i.clone(), *s, Type::Defer)
            }
            FieldSelectionElement::Macro(s, name, elem_name, expr, _) => {
                FieldSelectionElement::Macro(
                    *s,
                    name.clone(),
                    elem_name.clone(),
                    Box::new(AstExpr::shallow(expr)),
                    Type::Defer,
                )
            }
        }
    }

    fn pass(
        &mut self,
        type_env: &MappedArena<Type>,
        _annotation_env: &HashMap<String, AnnotationSpec>,
        scope: &Scope,
        errors: &mut Vec<Diagnostic>,
    ) -> bool {
        match self {
            FieldSelectionElement::Identifier(value, s, typ) => {
                if typ.is_incomplete() {
                    if value.as_str() == "self" {
                        if let Some(enclosing) = &scope.enclosing_type {
                            *typ = Type::Reference(type_env.get_id(enclosing).unwrap());
                            true
                        } else {
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
                        }
                    } else {
                        let context_type = type_env.get_by_key(value).and_then(|t| match t {
                            Type::Composite(c) if c.kind == AstModelKind::Context => Some(c),
                            _ => None,
                        });

                        if let Some(context_type) = context_type {
                            *typ = Type::Reference(type_env.get_id(&context_type.name).unwrap());
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
                        }
                        false
                    }
                } else {
                    false
                }
            }
            FieldSelectionElement::Macro(_, _, _, _, _) => {
                todo!()
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
                let updated = selection_elem.pass(type_env, annotation_env, scope, errors);
                match selection_elem {
                    FieldSelectionElement::Identifier(_, _, resolved_typ) => {
                        *typ = resolved_typ.clone();
                    }
                    FieldSelectionElement::Macro(_, _, _, _, _) => todo!(),
                }
                updated
            }
            FieldSelection::Select(prefix, i, _, typ) => {
                let in_updated = prefix.pass(type_env, annotation_env, scope, errors);
                let out_updated = if typ.is_incomplete() {
                    if let Type::Composite(c) = prefix.typ().deref(type_env) {
                        let i = match i {
                            FieldSelectionElement::Identifier(i, s, _) => (i, *s),
                            FieldSelectionElement::Macro(_, _, _, _, _) => todo!(),
                        };
                        if let Some(field) = c.fields.iter().find(|f| &f.name == i.0) {
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
                                message: format!("No such field {} on type {}", i.0, c.name),
                                code: Some("C000".to_string()),
                                spans: vec![SpanLabel {
                                    span: i.1,
                                    style: SpanStyle::Primary,
                                    label: Some("unknown field".to_string()),
                                }],
                            });
                            false
                        }
                    } else {
                        *typ = Type::Error;

                        let i = match i {
                            FieldSelectionElement::Identifier(i, s, _) => (i, *s),
                            FieldSelectionElement::Macro(_, _, _, _, _) => {
                                todo!()
                            }
                        };

                        if !prefix.typ().is_error() {
                            errors.push(Diagnostic {
                                level: Level::Error,
                                message: format!(
                                    "Cannot read field {} from a non-composite type {}",
                                    i.0,
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
                } else {
                    false
                };

                in_updated || out_updated
            }
        }
    }
}
