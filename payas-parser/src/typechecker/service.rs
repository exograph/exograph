use std::collections::HashMap;

use codemap_diagnostic::{Diagnostic, Level};
use payas_model::model::mapped_arena::MappedArena;

use crate::ast::ast_types::{
    AstArgument, AstFieldType, AstInterceptor, AstMethod, AstModel, AstModelKind, AstService,
    Untyped,
};

use super::{
    annotation::{AnnotationSpec, AnnotationTarget},
    AnnotationMap, Scope, Type, TypecheckFrom, Typed,
};

impl TypecheckFrom<AstService<Untyped>> for AstService<Typed> {
    fn shallow(untyped: &AstService<Untyped>) -> AstService<Typed> {
        let annotation_map = AnnotationMap::new(&untyped.annotations);

        AstService {
            name: untyped.name.clone(),
            models: untyped
                .models
                .iter()
                .map(|m| AstModel::shallow(m))
                .collect(),
            methods: untyped
                .methods
                .iter()
                .map(|m| AstMethod::shallow(m))
                .collect(),
            interceptors: untyped
                .interceptors
                .iter()
                .map(|m| AstInterceptor::shallow(m))
                .collect(),
            annotations: annotation_map,
            base_clayfile: untyped.base_clayfile.clone(),
        }
    }

    fn pass(
        &mut self,
        type_env: &payas_model::model::mapped_arena::MappedArena<super::Type>,
        annotation_env: &std::collections::HashMap<String, super::annotation::AnnotationSpec>,
        scope: &super::Scope,
        errors: &mut Vec<codemap_diagnostic::Diagnostic>,
    ) -> bool {
        if !self.annotations.contains("external") {
            errors.push(Diagnostic {
                level: Level::Error,
                message: format!("Missing @external annotation for service `{}`", self.name),
                code: Some("A000".to_string()),
                spans: vec![],
            })
        }

        let models_changed = self
            .models
            .iter_mut()
            .map(|m| {
                let model_scope = Scope {
                    enclosing_model: Some(m.name.clone()),
                };

                m.pass(type_env, annotation_env, &model_scope, errors)
            })
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

        let intercetor_changed = self
            .interceptors
            .iter_mut()
            .map(|m| m.pass(type_env, annotation_env, scope, errors))
            .filter(|v| *v)
            .count()
            > 0;

        let annot_changed = self.annotations.pass(
            AnnotationTarget::Service,
            type_env,
            annotation_env,
            scope,
            errors,
        );

        models_changed || methods_changed || intercetor_changed || annot_changed
    }
}

impl TypecheckFrom<AstMethod<Untyped>> for AstMethod<Typed> {
    fn shallow(untyped: &AstMethod<Untyped>) -> AstMethod<Typed> {
        let annotation_map = AnnotationMap::new(&untyped.annotations);

        AstMethod {
            name: untyped.name.clone(),
            typ: untyped.typ.clone(),
            arguments: untyped
                .arguments
                .iter()
                .map(|f| AstArgument::shallow(f))
                .collect(),
            return_type: AstFieldType::shallow(&untyped.return_type),
            is_exported: untyped.is_exported,
            annotations: annotation_map,
        }
    }

    fn pass(
        &mut self,
        type_env: &payas_model::model::mapped_arena::MappedArena<super::Type>,
        annotation_env: &std::collections::HashMap<String, super::annotation::AnnotationSpec>,
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
            arguments: untyped
                .arguments
                .iter()
                .map(|f| AstArgument::shallow(f))
                .collect(),
            annotations: annotation_map,
            span: untyped.span,
        }
    }

    fn pass(
        &mut self,
        type_env: &payas_model::model::mapped_arena::MappedArena<super::Type>,
        annotation_env: &std::collections::HashMap<String, super::annotation::AnnotationSpec>,
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
        if let Some(Type::Composite(model)) = type_env.get_by_key(&self.typ.name()) {
            if !matches!(model.kind, AstModelKind::NonPersistentInput) {
                errors.push(Diagnostic {
                    level: Level::Error,
                    message: format!(
                        "Cannot have non-input type in method argument `{}`",
                        self.name
                    ),
                    code: Some("A000".to_string()),
                    spans: vec![],
                })
            }
        }

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
