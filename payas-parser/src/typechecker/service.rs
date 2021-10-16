use std::collections::HashMap;

use codemap_diagnostic::{Diagnostic, Level};
use payas_model::model::mapped_arena::MappedArena;

use crate::ast::ast_types::{
    AstArgument, AstFieldType, AstMethod, AstModel, AstModelKind, AstService, Untyped,
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

        let annot_changed = self.annotations.pass(
            AnnotationTarget::Service,
            type_env,
            annotation_env,
            scope,
            errors,
        );

        models_changed || methods_changed || annot_changed
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
        let diagnostic = Diagnostic {
            level: Level::Error,
            message: format!(
                "Unknown method type `{}` for method `{}`",
                self.typ, self.name
            ),
            code: Some("A000".to_string()),
            spans: vec![],
        };

        match self.typ.as_ref() {
            "query" | "mutation" => {}
            _ => {
                errors.push(diagnostic);
            }
        }

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
