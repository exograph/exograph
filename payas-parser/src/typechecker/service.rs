use crate::ast::ast_types::{AstField, AstFieldType, AstMethod, AstModel, AstService, Untyped};

use super::{AnnotationMap, TypecheckFrom, Typed};

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
        // FIXME implement
        true
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
                .map(|f| AstField::shallow(f))
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
        // FIXME implement
        true
    }
}
