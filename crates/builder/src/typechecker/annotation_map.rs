// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::collections::{HashMap, hash_map};

use crate::{
    ast::ast_types::{AstAnnotation, AstAnnotationParams, Untyped},
    util,
};

use super::{Scope, TypecheckFrom};
use codemap::Span;
use codemap_diagnostic::{Diagnostic, Level, SpanLabel, SpanStyle};
use core_model::mapped_arena::MappedArena;
use core_model_builder::typechecker::{
    Typed,
    annotation::{AnnotationSpec, AnnotationTarget},
    annotation_map::AnnotationMap,
    typ::Type,
};

pub trait AnnotationMapImpl {
    fn new(ast_annotations: &[AstAnnotation<Untyped>]) -> Self;
    fn contains(&self, name: &str) -> bool;

    fn get(&self, name: &str) -> Option<&AstAnnotationParams<Typed>>;

    fn iter(&self) -> hash_map::Iter<'_, String, AstAnnotation<Typed>>;

    fn pass(
        &mut self,
        target: AnnotationTarget,
        type_env: &MappedArena<Type>,
        annotation_env: &HashMap<String, AnnotationSpec>,
        scope: &Scope,
        errors: &mut Vec<Diagnostic>,
    ) -> bool;
}

impl AnnotationMapImpl for AnnotationMap {
    fn new(ast_annotations: &[AstAnnotation<Untyped>]) -> Self {
        let mut annotations = HashMap::new();
        let mut spans: HashMap<String, Vec<Span>> = HashMap::new();

        for a in ast_annotations {
            match spans.get_mut(&a.name) {
                Some(spans) => spans.push(a.span),
                None => {
                    annotations.insert(a.name.clone(), AstAnnotation::shallow(a));
                    spans.insert(a.name.clone(), vec![a.span]);
                }
            }
        }

        AnnotationMap { annotations, spans }
    }

    fn contains(&self, name: &str) -> bool {
        self.annotations.contains_key(name)
    }

    fn get(&self, name: &str) -> Option<&AstAnnotationParams<Typed>> {
        self.annotations.get(name).map(|a| &a.params)
    }

    fn iter(&self) -> hash_map::Iter<'_, String, AstAnnotation<Typed>> {
        self.annotations.iter()
    }

    fn pass(
        &mut self,
        target: AnnotationTarget,
        type_env: &MappedArena<Type>,
        annotation_env: &HashMap<String, AnnotationSpec>,
        scope: &Scope,
        errors: &mut Vec<Diagnostic>,
    ) -> bool {
        for (name, spans) in &self.spans {
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
                    message: format!("Duplicate definitions of annotation `{name}`"),
                    code: Some("A000".to_string()),
                    spans: span_labels,
                });
            }
        }

        let mut changed = false;
        for annotation in self.annotations.values_mut() {
            match annotation_env.get(&annotation.name) {
                Some(spec) => {
                    let targets = spec.targets;
                    if !targets.contains(&target) {
                        let targets_str = util::join_strings(
                            &targets
                                .iter()
                                .map(|t| format!("{t:?}").to_lowercase())
                                .collect::<Vec<_>>(),
                            Some("or"),
                        );

                        errors.push(Diagnostic {
                            level: Level::Error,
                            message: format!("Invalid target for annotation `{}`", annotation.name),
                            code: Some("A000".to_string()),
                            spans: vec![SpanLabel {
                                span: annotation.span,
                                label: Some(format!("only applies to targets: {targets_str}")),
                                style: SpanStyle::Primary,
                            }],
                        });
                    }

                    let annot_changed = annotation.pass(type_env, annotation_env, scope, errors);
                    changed |= annot_changed;
                }
                None => {
                    errors.push(Diagnostic {
                        level: Level::Error,
                        message: format!("Unknown annotation `{}`", annotation.name),
                        code: Some("A000".to_string()),
                        spans: vec![SpanLabel {
                            span: annotation.span,
                            label: None,
                            style: SpanStyle::Primary,
                        }],
                    });
                    return false;
                }
            }
        }

        changed
    }
}
