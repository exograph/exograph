use anyhow::Result;
use payas_model::model::mapped_arena::MappedArena;

use crate::ast::ast_types::{AstField, AstModel, Untyped};

use super::{AnnotationMap, Scope, Type, TypecheckFrom, TypecheckInto, Typed};

impl TypecheckFrom<AstModel<Untyped>> for AstModel<Typed> {
    fn shallow(
        untyped: &AstModel<Untyped>,
        errors: &mut Vec<codemap_diagnostic::Diagnostic>,
    ) -> Result<AstModel<Typed>> {
        let mut annotations = Box::new(AnnotationMap::default());

        for a in &untyped.ast_annotations {
            let annotation = a.shallow(errors)?;
            annotations.add(errors, annotation, a.span)?;
        }

        Ok(AstModel {
            name: untyped.name.clone(),
            kind: untyped.kind.clone(),
            ast_annotations: untyped.ast_annotations.clone(),
            fields: untyped
                .fields
                .iter()
                .map(|f| AstField::shallow(f, errors))
                .collect::<Result<_, _>>()?,
            annotations,
        })
    }

    fn pass(
        &mut self,
        env: &MappedArena<Type>,
        _scope: &Scope,
        errors: &mut Vec<codemap_diagnostic::Diagnostic>,
    ) -> bool {
        let model_scope = Scope {
            enclosing_model: Some(self.name.clone()),
        };
        let fields_changed = self
            .fields
            .iter_mut()
            .map(|tf| tf.pass(env, &model_scope, errors))
            .filter(|v| *v)
            .count()
            > 0;

        let annot_changed = self
            .annotations
            .pass(&self.ast_annotations, env, &model_scope, errors);

        fields_changed || annot_changed
    }
}
