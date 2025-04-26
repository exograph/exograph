// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::collections::HashMap;

use codemap_diagnostic::{Diagnostic, Level};
use core_model::mapped_arena::MappedArena;
use core_model_builder::typechecker::{
    annotation::{AnnotationSpec, AnnotationTarget},
    annotation_map::AnnotationMap,
    Typed,
};

use crate::ast::ast_types::{
    AstArgument, AstFieldType, AstInterceptor, AstMethod, AstModule, Untyped,
};

use super::{annotation_map::AnnotationMapImpl, Scope, Type, TypecheckFrom};

fn typed<U, T: TypecheckFrom<U>>(untyped: &[U]) -> Vec<T> {
    untyped.iter().map(|u| T::shallow(u)).collect()
}

impl TypecheckFrom<AstModule<Untyped>> for AstModule<Typed> {
    fn shallow(untyped: &AstModule<Untyped>) -> AstModule<Typed> {
        let annotation_map = AnnotationMap::new(&untyped.annotations);

        AstModule {
            name: untyped.name.clone(),
            types: typed(&untyped.types),
            enums: typed(&untyped.enums),
            methods: typed(&untyped.methods),
            interceptors: typed(&untyped.interceptors),
            annotations: annotation_map,
            doc_comments: untyped.doc_comments.clone(),
            base_exofile: untyped.base_exofile.clone(),
            span: untyped.span,
        }
    }

    fn pass(
        &mut self,
        type_env: &MappedArena<Type>,
        annotation_env: &HashMap<String, AnnotationSpec>,
        scope: &super::Scope,
        errors: &mut Vec<codemap_diagnostic::Diagnostic>,
    ) -> bool {
        let types_changed = self
            .types
            .iter_mut()
            .map(|m| {
                let model_scope = Scope::with_enclosing_type(m.name.clone());

                m.pass(type_env, annotation_env, &model_scope, errors)
            })
            .filter(|v| *v)
            .count()
            > 0;

        let enums_changed = self
            .enums
            .iter_mut()
            .map(|e| e.pass(type_env, annotation_env, scope, errors))
            .filter(|v| *v)
            .count()
            > 0;

        let methods_changed = self
            .methods
            .iter_mut()
            .map(|m| m.pass(type_env, annotation_env, scope, errors))
            .filter(|v| *v)
            .count()
            > 0;

        let interceptor_changed = self
            .interceptors
            .iter_mut()
            .map(|m| m.pass(type_env, annotation_env, scope, errors))
            .filter(|v| *v)
            .count()
            > 0;

        let annot_changed = self.annotations.pass(
            AnnotationTarget::Module,
            type_env,
            annotation_env,
            scope,
            errors,
        );

        if self.annotations.annotations.is_empty() {
            errors.push(Diagnostic {
                level: Level::Error,
                message: format!(
                    "Module `{}` is not tagged with a subsystem annotation (which plugin should handle this?)",
                    self.name
                ),
                code: Some("A000".to_string()),
                spans: vec![],
            })
        }

        if self.annotations.annotations.len() > 1 {
            errors.push(Diagnostic {
                level: Level::Error,
                message: format!(
                    "Module `{}` is tagged with multiple subsystem annotations",
                    self.name
                ),
                code: Some("A000".to_string()),
                spans: vec![],
            })
        }

        types_changed || enums_changed || methods_changed || interceptor_changed || annot_changed
    }
}

impl TypecheckFrom<AstMethod<Untyped>> for AstMethod<Typed> {
    fn shallow(untyped: &AstMethod<Untyped>) -> AstMethod<Typed> {
        let annotation_map = AnnotationMap::new(&untyped.annotations);

        AstMethod {
            name: untyped.name.clone(),
            typ: untyped.typ.clone(),
            arguments: typed(&untyped.arguments),
            return_type: AstFieldType::shallow(&untyped.return_type),
            is_exported: untyped.is_exported,
            annotations: annotation_map,
            doc_comments: untyped.doc_comments.clone(),
            span: untyped.span,
        }
    }

    fn pass(
        &mut self,
        type_env: &MappedArena<Type>,
        annotation_env: &HashMap<String, AnnotationSpec>,
        scope: &super::Scope,
        errors: &mut Vec<codemap_diagnostic::Diagnostic>,
    ) -> bool {
        let arguments_changes = self
            .arguments
            .iter_mut()
            .map(|a| a.pass(type_env, annotation_env, scope, errors))
            .filter(|v| *v)
            .count()
            > 0;

        let return_type_change = self
            .return_type
            .pass(type_env, annotation_env, scope, errors);

        let annot_changed = self.annotations.pass(
            AnnotationTarget::Method,
            type_env,
            annotation_env,
            scope,
            errors,
        );

        arguments_changes || return_type_change || annot_changed
    }
}

impl TypecheckFrom<AstInterceptor<Untyped>> for AstInterceptor<Typed> {
    fn shallow(untyped: &AstInterceptor<Untyped>) -> AstInterceptor<Typed> {
        let annotation_map = AnnotationMap::new(&untyped.annotations);

        AstInterceptor {
            name: untyped.name.clone(),
            arguments: typed(&untyped.arguments),
            annotations: annotation_map,
            doc_comments: untyped.doc_comments.clone(),
            span: untyped.span,
        }
    }

    fn pass(
        &mut self,
        type_env: &MappedArena<Type>,
        annotation_env: &HashMap<String, AnnotationSpec>,
        scope: &super::Scope,
        errors: &mut Vec<codemap_diagnostic::Diagnostic>,
    ) -> bool {
        let has_a_valid_annotation = ["before", "after", "around"]
            .iter()
            .any(|valid_annotaiton| self.annotations.contains(valid_annotaiton));

        if !has_a_valid_annotation {
            errors.push(Diagnostic {
                level: Level::Error,
                message: format!(
                    "Missing @before/@after/@around annotation for interceptor `{}`",
                    self.name
                ),
                code: Some("A000".to_string()),
                spans: vec![],
            })
        }

        let arguments_changes = self
            .arguments
            .iter_mut()
            .map(|a| a.pass(type_env, annotation_env, scope, errors))
            .filter(|v| *v)
            .count()
            > 0;

        let annot_changed = self.annotations.pass(
            AnnotationTarget::Interceptor,
            type_env,
            annotation_env,
            scope,
            errors,
        );

        arguments_changes || annot_changed
    }
}

impl TypecheckFrom<AstArgument<Untyped>> for AstArgument<Typed> {
    fn shallow(untyped: &AstArgument<Untyped>) -> AstArgument<Typed> {
        let annotation_map = AnnotationMap::new(&untyped.annotations);

        AstArgument {
            name: untyped.name.clone(),
            typ: AstFieldType::shallow(&untyped.typ),
            annotations: annotation_map,
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
            AnnotationTarget::Argument,
            type_env,
            annotation_env,
            scope,
            errors,
        );

        typ_changed || annot_changed
    }
}
