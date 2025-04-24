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
    ast::ast_types::{AstEnum, AstEnumField, AstFragmentReference},
    typechecker::{
        annotation::{AnnotationSpec, AnnotationTarget},
        annotation_map::AnnotationMap,
        Typed,
    },
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
            fragment_references: untyped
                .fragment_references
                .iter()
                .map(AstFragmentReference::shallow)
                .collect(),
            annotations: annotation_map,
            doc_comments: untyped.doc_comments.clone(),
            span: untyped.span,
        }
    }

    fn pass(
        &mut self,
        type_env: &MappedArena<Type>,
        annotation_env: &HashMap<String, AnnotationSpec>,
        _scope: &Scope,
        errors: &mut Vec<Diagnostic>,
    ) -> bool {
        let model_scope = Scope::with_enclosing_type(self.name.clone());

        let fields_changed = self
            .fields
            .iter_mut()
            .map(|tf| tf.pass(type_env, annotation_env, &model_scope, errors))
            .filter(|v| *v)
            .count()
            > 0;

        let fragment_references_changed = self
            .fragment_references
            .iter_mut()
            .map(|fr| fr.pass(type_env, annotation_env, &model_scope, errors))
            .filter(|v| *v)
            .count()
            > 0;

        if matches!(self.kind, AstModelKind::Context) {
            self.fields.iter().for_each(|field| {
                if let Some(AstFieldDefault { span, .. }) = &field.default_value {
                    errors.push(Diagnostic {
                        level: Level::Error,
                        message: "Default fields can only be specified in types".to_string(),
                        code: Some("C000".to_string()),
                        spans: vec![SpanLabel {
                            span: *span,
                            style: SpanStyle::Primary,
                            label: Some("bad default field".to_string()),
                        }],
                    });
                }
            })
        };

        let annot_changed = self.annotations.pass(
            AnnotationTarget::Type,
            type_env,
            annotation_env,
            &model_scope,
            errors,
        );

        fields_changed || annot_changed || fragment_references_changed
    }
}

impl TypecheckFrom<AstEnum<Untyped>> for AstEnum<Typed> {
    fn shallow(untyped: &AstEnum<Untyped>) -> AstEnum<Typed> {
        AstEnum {
            name: untyped.name.clone(),
            fields: untyped.fields.iter().map(AstEnumField::shallow).collect(),
            doc_comments: untyped.doc_comments.clone(),
            span: untyped.span,
        }
    }

    fn pass(
        &mut self,
        _type_env: &MappedArena<Type>,
        _annotation_env: &HashMap<String, AnnotationSpec>,
        _scope: &Scope,
        _errors: &mut Vec<Diagnostic>,
    ) -> bool {
        false
    }
}

impl TypecheckFrom<AstEnumField<Untyped>> for AstEnumField<Typed> {
    fn shallow(untyped: &AstEnumField<Untyped>) -> AstEnumField<Typed> {
        AstEnumField {
            name: untyped.name.clone(),
            typ: true,
            doc_comments: untyped.doc_comments.clone(),
            span: untyped.span,
        }
    }

    fn pass(
        &mut self,
        _type_env: &MappedArena<Type>,
        _annotation_env: &HashMap<String, AnnotationSpec>,
        _scope: &Scope,
        _errors: &mut Vec<Diagnostic>,
    ) -> bool {
        false
    }
}

impl TypecheckFrom<AstFragmentReference<Untyped>> for AstFragmentReference<Typed> {
    fn shallow(untyped: &AstFragmentReference<Untyped>) -> AstFragmentReference<Typed> {
        AstFragmentReference {
            name: untyped.name.clone(),
            typ: false,
            doc_comments: untyped.doc_comments.clone(),
            span: untyped.span,
        }
    }
    fn pass(
        &mut self,
        type_env: &MappedArena<Type>,
        _annotation_env: &HashMap<String, AnnotationSpec>,
        _scope: &Scope,
        _errors: &mut Vec<Diagnostic>,
    ) -> bool {
        if self.typ {
            return false;
        }

        let fragment_type = type_env.get_by_key(&self.name);

        if let Some(Type::Composite(_)) = fragment_type {
            self.typ = true;
            true
        } else {
            false
        }
    }
}
