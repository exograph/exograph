use std::collections::HashMap;

use codemap_diagnostic::{Diagnostic, Level};
use payas_core_model::mapped_arena::MappedArena;
use payas_core_model_builder::typechecker::{
    annotation::{AnnotationSpec, AnnotationTarget},
    annotation_map::AnnotationMap,
    Typed,
};

use crate::ast::ast_types::{
    AstArgument, AstFieldType, AstInterceptor, AstMethod, AstModelKind, AstService, Untyped,
};

use super::{annotation_map::AnnotationMapImpl, Scope, Type, TypecheckFrom};

fn typed<U, T: TypecheckFrom<U>>(untyped: &[U]) -> Vec<T> {
    untyped.iter().map(|u| T::shallow(u)).collect()
}

impl TypecheckFrom<AstService<Untyped>> for AstService<Typed> {
    fn shallow(untyped: &AstService<Untyped>) -> AstService<Typed> {
        let annotation_map = AnnotationMap::new(&untyped.annotations);

        AstService {
            name: untyped.name.clone(),
            models: typed(&untyped.models),
            methods: typed(&untyped.methods),
            interceptors: typed(&untyped.interceptors),
            annotations: annotation_map,
            base_clayfile: untyped.base_clayfile.clone(),
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
            arguments: typed(&untyped.arguments),
            return_type: AstFieldType::shallow(&untyped.return_type),
            is_exported: untyped.is_exported,
            annotations: annotation_map,
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
        if let Some(Type::Composite(model)) = type_env.get_by_key(&self.typ.name()) {
            match model.kind {
                AstModelKind::NonPersistentInput | AstModelKind::Context => {}
                _ => errors.push(Diagnostic {
                    level: Level::Error,
                    message: format!(
                        "Argument `{}` must be either an input type or a context",
                        self.name
                    ),
                    code: Some("A000".to_string()),
                    spans: vec![],
                }),
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
