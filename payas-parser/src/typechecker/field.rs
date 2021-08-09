use anyhow::Result;
use payas_model::model::mapped_arena::MappedArena;

use crate::ast::ast_types::{AstField, Untyped};

use super::{AnnotationMap, Scope, Type, TypecheckFrom, TypecheckInto, Typed};

impl TypecheckFrom<AstField<Untyped>> for AstField<Typed> {
    fn shallow(
        untyped: &AstField<Untyped>,
        errors: &mut Vec<codemap_diagnostic::Diagnostic>,
    ) -> Result<AstField<Typed>> {
        let mut annotations = Box::new(AnnotationMap::default());

        for a in &untyped.ast_annotations {
            let annotation = a.shallow(errors)?;
            annotations.add(errors, annotation, a.span)?;
        }

        Ok(AstField {
            name: untyped.name.clone(),
            ast_typ: untyped.ast_typ.clone(),
            typ: untyped.ast_typ.shallow(errors)?,
            ast_annotations: untyped.ast_annotations.clone(),
            annotations,
        })
    }

    fn pass(
        &mut self,
        env: &MappedArena<Type>,
        scope: &Scope,
        errors: &mut Vec<codemap_diagnostic::Diagnostic>,
    ) -> bool {
        let typ_changed = self.ast_typ.pass(&mut self.typ, env, scope, errors);

        let annot_changed = self
            .annotations
            .pass(&self.ast_annotations, env, scope, errors);

        typ_changed || annot_changed
    }
}
