use anyhow::Result;
use payas_model::model::mapped_arena::MappedArena;

use crate::ast::ast_types::{AstModel, AstModelKind, Untyped};

use super::{typ::CompositeTypeKind, AnnotationMap, CompositeType, Scope, Type, Typecheck};

impl Typecheck<Type> for AstModel<Untyped> {
    fn shallow(&self, errors: &mut Vec<codemap_diagnostic::Diagnostic>) -> Result<Type> {
        let mut annotations = Box::new(AnnotationMap::default());

        for a in &self.annotations {
            let annotation = a.shallow(errors)?;
            annotations.add(errors, annotation, a.span)?;
        }

        Ok(Type::Composite(CompositeType {
            name: self.name.clone(),
            kind: if self.kind == AstModelKind::Persistent {
                CompositeTypeKind::Persistent
            } else {
                CompositeTypeKind::Context
            },
            fields: self
                .fields
                .iter()
                .map(|f| f.shallow(errors))
                .collect::<Result<_, _>>()?,
            annotations,
        }))
    }

    fn pass(
        &self,
        typ: &mut Type,
        env: &MappedArena<Type>,
        _scope: &Scope,
        errors: &mut Vec<codemap_diagnostic::Diagnostic>,
    ) -> bool {
        if let Type::Composite(c) = typ {
            let model_scope = Scope {
                enclosing_model: Some(self.name.clone()),
            };
            let fields_changed = self
                .fields
                .iter()
                .zip(c.fields.iter_mut())
                .map(|(f, tf)| f.pass(tf, env, &model_scope, errors))
                .filter(|v| *v)
                .count()
                > 0;

            let annot_changed = c
                .annotations
                .pass(&self.annotations, env, &model_scope, errors);

            fields_changed || annot_changed
        } else {
            panic!()
        }
    }
}
