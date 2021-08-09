use anyhow::Result;
use payas_model::model::mapped_arena::MappedArena;

use crate::ast::ast_types::{AstField, AstModel, Untyped};

use super::{AnnotationMap, Scope, Type, TypecheckFrom, Typed, TypedAnnotation};

impl TypecheckFrom<AstModel<Untyped>> for AstModel<Typed> {
    fn shallow(
        untyped: &AstModel<Untyped>,
        errors: &mut Vec<codemap_diagnostic::Diagnostic>,
    ) -> Result<AstModel<Typed>> {
        let mut annotations = Box::new(AnnotationMap::default());

        for a in &untyped.annotations {
            let annotation = TypedAnnotation::shallow(a, errors)?;
            annotations.add(errors, annotation, a.span)?;
        }

        Ok(AstModel {
            name: untyped.name.clone(),
            kind: untyped.kind.clone(),
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

        let annot_changed = self.annotations.pass(env, &model_scope, errors);

        fields_changed || annot_changed
    }
}
